# UI specification

Status: M1 spec. Package: `ui/` (React 19, TypeScript strict, Vite, Tailwind CSS 4, zustand, lucide-react, clsx; linted with oxlint and formatted with oxfmt). Read with [architecture.md](../design/architecture.md) (section "UI"), [technical design](../design/technical-design.md) sections 7, 8, 11, 13 and 14, ADRs [0002](../adr/0002-react-for-the-ui.md), [0006](../adr/0006-protocol-framing-and-schema.md), [0007](../adr/0007-webgl2-renderer-first.md), [0009](../adr/0009-emscripten-c-abi-bridge.md), [0013](../adr/0013-oxc-tooling-and-theme-stylesheet.md), [0030](../adr/0030-islands-over-a-full-window-viewport-and-rgb-axes.md), [0031](../adr/0031-self-hosted-geist-and-jetbrains-mono-for-code.md) and [0032](../adr/0032-flat-islands-without-outlines.md), the guidelines in [docs/guidelines/](../guidelines/README.md) ([TypeScript](../guidelines/typescript.md), [Tailwind CSS](../guidelines/tailwind-css.md), [frontend design](../guidelines/frontend-design.md) and the Partsmith design direction), and the specs this one consumes: [protocol.md](protocol.md) (wire, commands, events, worker host, frame codec), [features.md](features.md) (manifests, shared param types, labels, messages), [model.md](model.md) (edits, ids, labels) and sketch.md and print.md (sketch and printer JSON).

Lost references, one-click fixes, causes, repair mode and the prevention measures of 3.7, 3.9, 3.16.10, 3.21, 3.22 and 4 follow [the reference stability and errors study](../design/reference-stability-and-errors.md) section 6 and ADRs [0021](../adr/0021-resolve-references-by-meaning.md), [0023](../adr/0023-frames-live-on-the-model.md) and [0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md); the messages this spec adds for them follow [the error guidelines](../guidelines/errors.md) (older UI-owned texts differ in one point, open issue 19).

This spec is the definition of done for the UI in M1. Test authors write the tests in section 6 from it without seeing the implementation; implementers make them pass without weakening them. Where this spec quotes another spec's JSON or names, the other spec is authoritative and a mismatch is a bug in this spec.

## 1. Purpose and scope

The UI is the single front end for web and desktop. It renders the model, edits it with the mouse and keyboard, and talks to the core only through protocol frames (architecture.md: "The UI depends only on the protocol"). It never evaluates geometry, never parses `.ptsm` text (except `referencedAssetNames`, 2.15, which collects the string values that match `ASSET_NAME` to choose which asset files to read and interprets nothing else; ADR 0037 decision 2, needs human review) and never constructs a face, edge or vertex reference itself: references are copied from protocol data (`pick_reference` results, sketch solutions), and the typed wrappers around them (`PlaneRef`, `AxisRef`, `ProfileRef`) are assembled exactly as features.md section 2.5 defines. Display arithmetic on values the core reports is not geometry evaluation: a diameter from a descriptor's radius (3.8) or the point where the pointer's ray meets a reported face plane (3.7, as sketch mode does for its plane, 3.16.3).

### 1.1 In M1 (the "M1 slice")

- App shell: a viewport that fills the window, with the top bar, collapsible browser, timeline, right side panel and status bar floating over it as islands (3.2), floating inspector, notifications, X, Y and Z in red, green and blue wherever an axis or a coordinate is shown, light and dark themes from one role-to-color mapping toggled with the `dark` class on the root element (2.16), UI scale 100 to 300 percent.
- Three contexts (Model, Sketch, Print), each with one short toolbar, and one command registry shared by toolbars, the command palette and keyboard shortcuts.
- Command palette (Ctrl/Cmd+K) with fuzzy search over commands, features, bodies, parameters, recent files and settings, showing each command's shortcut and description, finding commands by their Fusion and FreeCAD names, and browsable by category with an empty query.
- Timeline: rollback marker drag, drag to reorder with a warning before a drop that breaks what a feature uses, suppress, status icons and note marks, double-click to edit, rename, delete, "Used by" in each item's menu, and a Problems list grouped by root cause with notes (3.9).
- Browser: origin, bodies, sketches, datum planes and parameters, with eye, lock and color controls.
- Schema-driven inspector for every catalog feature: essential fields and a "More" toggle from manifest hints (plus the inline Reverse of 2.12), starting values that work on printed parts, expression inputs with live validation, unit display and parameter name completion, selection fields that capture viewport and browser picks, live preview through protocol transactions, undo inside a command, Enter confirms, Esc cancels.
- Parameters panel.
- WebGL2 viewport: matcap-style shaded faces, screen-space anti-aliased edges, ID-buffer picking of faces, edges and vertices with hover highlight, selection filter, section plane, analysis color overlay, ground grid, view cube, orbit, pan and zoom with Fusion and FreeCAD navigation presets (both also orbit with a right drag), a right-click context menu, fit to view, perspective and orthographic projection.
- Sketch mode for points, lines, circles and arcs: tools line/polyline, rectangle (2-point and center), circle (center, radius), arc (3-point), point, construction toggle; constraint tools coincident, point on object, horizontal, vertical, parallel, perpendicular, tangent, collinear, concentric, equal, symmetric, midpoint, fix; dimension tool for distance, horizontal and vertical distance, radius, diameter and angle with expression entry; auto-constraint inference while drawing (coincident, horizontal, vertical, tangent, midpoint) with a hold key to suppress it; dimension as you draw; step prompts; DOF colors and DOF count; constraint glyphs; point drag solved by the core at up to 60 fps; profile check display.
- Repair mode for lost references and regions (3.7): the lost element's ghost drawn where it was in its last build, numbered candidates described by meaning and distance, one-click use of a candidate, re-pick on the canvas, Remove from list, Keep this match for followed references, a notification for every new loss (3.22), and the cause line of every message with its actions (4.1). This is the M1 answer to "references that survive edits": when one cannot, the user is told at once, sees what was lost and why, and fixes it in one click.
- One-click fixes and prevention from the core (4.2): every message's actions through `apply_fix`, Copy details, the largest working value under fillet, chamfer and shell fields (3.7), Keep sizes when the document units change (3.21), an Extrude and Revolve availability reason that names the sketch whose outline is open, with Show (2.10), and marks on sketch entities that later features use (3.16.10).
- Print context: printer profile picker, check toggles, overhang color map with legend, print check badge and list with zoom to problem, bed outline, and Lay on face for print checks and 3MF and STL export.
- Guidance inside the one interface (open issue 15): the empty-document first step, descriptions and field help on hover, availability reasons before a click, next steps and measurements in the selection inspector, and the keyboard and mouse reference (F1).
- Export to 3MF, STL (binary), OBJ and STEP.
- Files: new, open, save, save as with the File System Access API and a download fallback, drag-and-drop open, recent files and a recovery copy per document session in IndexedDB; a save that stops when the file was changed elsewhere; and the `assets/` folder next to a document with imports (written by Python), which Open reads and Save writes (3.19).
- Feature types covered through the generic inspector: `sketch`, `extrude`, `revolve`, `hole`, `box`, `cylinder`, `sphere`, `cone`, `torus`, `fillet`, `chamfer`, `shell`, `boolean`, `move_body`, `split_body`, `mirror`, `linear_pattern`, `circular_pattern`, `datum_plane` (features.md section 3.13). The `import` type has no command in M1 (2.10); its timeline items are shown and managed like any other feature.
- Accessibility: WCAG 2.2 AA text and non-text contrast, full keyboard operation of every panel, landmarks, live regions.
- Worker and Tauri transports, the core client, and the generated protocol types.
- Browsers and webviews (ADR 0038; the minimums need human review): the current releases of Chrome, Edge, Firefox and Safari on desktop computers, and the webviews of the desktop app (desktop.md 1), each from the version the table below gives, the first with every web feature the UI relies on. Every E-test runs in Chromium on every change, and the tests tagged `@engines` run in Firefox and WebKit every week (6.8, delivery.md 2.14).

| Browser or webview | Minimum | Tested in |
| --- | --- | --- |
| Chrome, Edge | 114 | Every E-test, in Playwright's Chromium 156 (`chromium` project, CI's `e2e` job) |
| Firefox | 128 | The `@engines` tests, in Playwright's Firefox 157 (`firefox` project, weekly) |
| Safari | 17.0 | The `@engines` tests, in Playwright's WebKit 27.2 on Linux (`webkit` project, weekly) |
| WebView2, the desktop app on Windows (x64) | 114, the Evergreen runtime, which the installer adds when it is missing and Windows keeps current | The desktop checks at the gate (desktop.md 6) |
| WKWebView, the desktop app on macOS 13 or later | The WebKit of Safari 17.0, which the webview takes from the installed Safari | The desktop checks at the gate |
| WebKitGTK 4.1, the desktop app on Linux | 2.44, the release Ubuntu 24.04 shipped | The desktop checks at the gate; the `@engines` tests in Playwright's WebKit on Linux are the nearest |

The minimums are the first versions of Chrome, Firefox and Safari that have each feature below (MDN's compatibility data; CI tests only the versions of the table's last column):

- The Popover API (`showPopover`, `:popover-open`), which every menu, popover and tooltip uses (2.10, 3.2): 114, 125 and 17.0.
- The stylesheet of Tailwind CSS 4 (cascade layers, `@property`, `color-mix()`): 111, 128 and 16.4.
- What the web core needs: WebAssembly with exception handling as Emscripten 4.0.15 emits it by default (the legacy proposal), a module worker and cross-origin isolation: 95, 114 and 15.2. The desktop app runs the core natively and needs none of them.
- Web Locks, `crypto.randomUUID`, `structuredClone` and the array methods that copy, such as `toSorted` (2.15, 3.19): 110, 115 and 16.0 at most.

Two features are used where they exist and done without elsewhere: `@starting-style` (the entry transitions of popups; 117, 129 and 17.5) and the `source` option of `showPopover`, which keeps a click on a popup's own anchor from closing it before the anchor toggles it. Phones and tablets are not supported (1.2, touch-optimized layout). Below the minimums the app is not supported, and the unsupported page (3.1) checks only WebAssembly and WebGL2 (open issue 40).

### 1.2 Later (not in M1)

- Onboarding tutorial and looping tool-tip animations. The "Coming from Fusion / FreeCAD" mapping is part of M1 as the palette's tagged keywords (2.10), not as a separate panel.
- A curated sample part opened from the empty-state card (3.2).
- Right-click radial marking menu in the viewport (M1 has a plain context menu with the same items, 3.13).
- 3Dconnexion SpaceMouse.
- Code panel (CodeMirror 6), script features, Python export UI and the Customizer panel.
- On-canvas handles for feature values (technical design 13, principle 1): an arrow for Extrude distance, Hole depth, Datum plane offset, Shell thickness and Fillet and Chamfer size, an arc for Revolve angle and a triad for Move body, each with a value chip that is the field's `ExpressionInput`. Dragging writes plain numbers in `stepExpression` steps (Shift coarse, Alt fine) through the preview `LatestOnly`, dragging an extrude arrow through zero toggles Reverse, and a field holding an expression that is not a plain number shows a dimmed handle that does not drag. Needs a handle anchor (point and direction per field path) from the core with the preview.
- WebGPU render backend.
- Multi-document tabs.
- Dependency marks drawn on every timeline item, and reorder drops that refuse to break a dependency. M1 warns while a drop would break a use and lists "Used by" in the item's menu, from `list_uses` (3.9); a drop is never refused.
- Navigation presets SolidWorks, Blender and Onshape; trackpad gestures beyond pinch zoom and the secondary-button drag of 3.13 (two-finger scroll to pan or orbit); touch-optimized layout.
- FreeCAD shortcut preset, user shortcut rebinding and user-pinned toolbar tools.
- Remaining sketch tools (3-point and rounded rectangle, 2-point, 3-point and tangent circle, center and tangent arc, tangent-arc toggle in the line tool, conics, splines, polygon, slots, text, image, DXF and SVG import), modify tools (trim, extend, split, sketch fillet and chamfer, offset, mirror, patterns, move and copy) and reference geometry (project, intersect, carbon copy).
- Remaining constraints (smooth, block, arc length, angle to an axis as its own type, Snell's law, B-spline constraints).
- Dragging sketch curves (translating a line, resizing a circle); M1 drags points only.
- Face edges of the model as sketch snap and dimension targets (sketch.md and engine changes).
- Window and crossing box selection in the Model context (M1 has it in sketch mode only).
- Keyboard picking of faces, edges and vertices.
- Features outside the slice (sweep, loft, helix, rib, emboss, thread, draft, scale, direct edits, derive, surfaces, mesh bodies, configurations), components, joints and assemblies.
- Measure tool, distances and angles between two selected items (the core's two-target `measure`, protocol.md 1.2), and mass properties beyond volume.
- Import UI (STEP, meshes, `.scad`, `.FCStd`, DXF, SVG) and `.ptsz` bundles.
- Print-aware feature presets, orientation tools beyond Lay on face (auto-orient, saving the orientation in the document), custom printer profiles (`put_printer_profile`, with a decision on documents that name a profile another machine lacks), slicer handoff and watch mode, resin tools, heat maps for checks other than overhangs, multi-plate arrangement, and applying a print suggestion in one click (split into bodies, drain hole; M1 words, frames and draws it, 3.17, ADR 0040).
- Export options: one file per body and a mesh tolerance (M1 exports one file with the profile's tolerance, 3.18).
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
- **Settled recipe**: the recipe of the latest `document_changed` with `transactionOpen` false: every edit the user has confirmed and none of the preview edits of an open feature command (3.7). The recovery copy (3.19) and a core restart (4.4) keep it.
- **Context**: Model, Sketch or Print.
- **DOF**: remaining degrees of freedom of a sketch.

## 2. Public API and module structure

### 2.1 Layout of `ui/`

```text
ui/
  index.html                 color-scheme meta, the Geist preload (2.16.7, 2.16.8)
  public/fonts/              the two self-hosted fonts and their licenses (2.16.7)
  scripts/generate-protocol-types.mjs   protocol types generator (2.2)
  src/
    index.css                Tailwind import, font faces, font theme variables, dark variant (ADRs 0013 and 0031;
                             nothing else, 2.16.1)
    main.tsx                 capability check, theme bootstrap, createApp, render
    App.tsx                  layout shell (regions of section 3.2)
    app/
      createApp.ts           createApp, App
      AppProvider.tsx        React context; useApp, useAppState
      actions/               document.ts, selection.ts, command.ts, context.ts, sketch.ts,
                             timeline.ts, bodies.ts, parameters.ts, print.ts, export.ts,
                             files.ts, notifications.ts, fixes.ts
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
      queue.ts               createMutationQueue, createLatestOnly, skipRepeats
      ids.ts                 generateId
      labels.ts              featureLabel, nextBodyName, originPlaneView, originPlaneName
    generated/
      protocol.ts            generated, checked in, never edited
    state/
      store.ts               AppState, AppStore, createAppStore
      selectors.ts
    commands/
      registry.ts            CommandRegistry, createCommandRegistry
      shortcuts.ts           parseShortcut, chordToString, formatShortcut, chordFromEvent, ariaKeyShortcut
      dispatcher.ts          installShortcutDispatcher
      fuzzy.ts               fuzzyScore, rankPaletteItems
      palette.ts             paletteItems
      builtin.ts             built-in UI commands
      features.ts            commands generated from the feature catalog, FEATURE_REQUIREMENTS
      keywords.ts            COMMAND_KEYWORDS
      descriptions.ts        FEATURE_DESCRIPTIONS
      component-keys.ts      COMPONENT_KEYS, TYPING_KEYS, TYPING_NOTE (the keys of 3.5 as data), withPlatformKeys
      toolbars.ts            TOOLBARS
      icons.ts               FEATURE_ICONS, constraint glyph icons
    inspector/
      model.ts               buildInspectorModel, defaultParams, setParam, getParam
      start-values.ts        START_VALUES
      help.ts                FIELD_HELP, fieldHelp
      suggestions.ts         suggestedCommands
      measurements.ts        selectionMeasurements, measureSelection, volumeMeter, formatVolume
      entity-size.ts         entitySize, entitySizeDimension (the size field, 3.8)
      selection-values.ts    selectionValue, acceptsTarget, itemsFromValue
      placement.ts           placeInspector
      Inspector.tsx, fields/*.tsx
    expression/
      evaluate.ts            convertForField
      format.ts              formatNumber, formatQuantity, formatSize, formatPoint
      completion.ts          completionNames, completionAt, insertCompletion, missingParameterName (2.13)
      step.ts                stepExpression, isPlainNumber
      span.ts                utf16Range
      ExpressionInput.tsx, useExpression.ts
    viewport/
      math.ts                Vector3, Matrix4, Quaternion helpers
      camera.ts              Camera and camera functions
      navigation.ts          NAVIGATION_PRESETS, gestureFor, navigationLegend
      grid.ts                gridSpacing, gridPlacement, drawnGridSpacing, the grid's lines
      menu.ts                viewportMenuItems
      backend.ts             RenderBackend and its types
      webgl2/                createWebGl2Backend and shaders
      picking.ts             encodePickId, decodePickId, choosePick
      scene.ts               Scene, createScene
      element-extent.ts      elementExtentPoints (what the inspector opens beside, 3.7)
      controller.ts          ViewportController, ViewportHandle
      overlay.ts             positionElement, orientElement (runtime positions, 3.12)
      canvas-layer.ts        CanvasLayerContext, useCanvasLayer (the canvas-aligned host of 3.2)
      Viewport.tsx, ViewCube.tsx, ViewportControls.tsx, SectionPopover.tsx, EmptyState.tsx, ViewportMenu.tsx,
      TopCenterSlot.tsx
    sketch/
      refs.ts                sketch origin and axis references (sketch.md)
      geometry.ts            circleThroughThreePoints, tessellateArc, distance helpers
      recipes.ts             tool outputs
      inference.ts           infer
      constraints.ts         applicableConstraints, constraintFor
      dimensions.ts          dimensionTypeFor, measuredValue, defaultLabelPosition
      session.ts             SketchToolId, SketchChange, SketchCommitResult, SketchStep, sketchStepHint,
                             the internal sketch controller
      issues.ts              profile finding messages and rings, outlineProblems
      overlay-chunk.ts       loadSketchOverlay, loadedSketchOverlay (3.1)
      SketchOverlay.tsx, SketchHeader.tsx, DrawInputs.tsx, DimensionEditor.tsx
    timeline/ Timeline.tsx, FeatureMenu.tsx, DropHint.tsx, uses.ts (dropWarning, usersOf, usedEntities,
              usedByText)
    repair/ repair.ts (repairView, candidateAction, ghostSegments), RepairOverlay.tsx, CauseLine.tsx
    browser/ Browser.tsx
    parameters/ ParametersPanel.tsx
    print/ PrinterMenu.tsx, ChecksPopover.tsx, HealthList.tsx, HealthBadge.tsx, OverhangLegend.tsx,
           PrintHint.tsx, printer.ts (printerDetail, printerButtonText),
           health.ts (healthRows, findingPoint, healthBadgeText, healthBadgeName), orientation.ts
           (sameOrientation, selectOrientationBody), suggestion.ts (suggestionText, splitPlaneOutline,
           focusedSplitOutlines)
    export/ ExportPopover.tsx, status.ts (exportStatus)
    problems/ ProblemsPanel.tsx, problems.ts (problemRows, changeNotifications)
    keys/ KeysPanel.tsx
    files/
      file-service.ts        FileService, createWebFileService
      recent.ts              RecentFilesStore, createIndexedDbRecentFiles, createMemoryRecentFiles
      recovery.ts            RecoveryStore, createIndexedDbRecovery, createMemoryRecovery
    settings/ settings.ts    Settings, loadSettings, saveSettings
    theme/
      roles.ts               the role-to-color mapping, palette hex values, viewport colors,
                             UI scale classes (2.16)
      theme.ts               resolveTheme, applyTheme, applyUiScale, toggledTheme
    layout/ TopBar.tsx, StatusBar.tsx, SidePanel.tsx, Notifications.tsx, DocumentMenu.tsx,
            docking.ts (DOCKING_WIDTH_REM, isNarrowWindow, startsNarrow: the 65rem rule of 3.2)
    components/              Button, IconButton, Menu, Popover, Tooltip, Switch, SegmentedControl, TextField,
                             HelpLabel (a label with hover help, 3.7), AxisLetter and AxisValues (axis letters
                             and coordinate readouts, 3.7, 3.8), described-by.ts
    performance/marks.ts     TIMING_NAMES, markTiming, measureTiming
    messages.ts              UI-owned message catalog (section 4.3)
    test/ setup.ts, fakes/   FakeTransport, FakeCoreApi, FakeBackend, the ResizeObserver stand-in
                             (stubResizeObserver), catalog and documents fixtures
  e2e/
    fixtures/                .ptsm fixtures (6.2)
    helpers.ts               shared Playwright helpers (6.3)
    *.spec.ts
```

Module rules:

- Only `src/core/api.ts` names protocol commands and only `src/core/events.ts` names protocol events. Every other module uses `CoreApi` and `CoreEvent`.
- React components never call `CoreApi`; they call actions (2.9). Actions are plain functions taking the `App`, so they are unit-testable with fakes.
- The viewport and sketch drag bypass React (ADR 0002): `ViewportController` subscribes to the store with `store.subscribe` and draws on demand. React renders only DOM chrome and DOM overlays.
- Runtime dependencies are exactly those in `ui/package.json` (`react`, `react-dom`, `zustand`, `lucide-react`, `clsx`). Menus, popovers, tooltips and the fuzzy matcher are written in-house; popovers use the native Popover API. Icons come from `lucide-react` only; there are no charts in M1. Motion uses Tailwind transition utilities only; the `motion` library is not a dependency (adding it, like any dependency, needs human review, AGENTS.md).
- Hand-written code never redeclares a type that exists in `src/generated/protocol.ts`. The only mirror types are for protocol values the schema carries as untyped JSON (feature params schemas only), and they are validated at runtime (2.12).
- TypeScript follows [typescript.md](../guidelines/typescript.md): `type`, never `interface` (oxlint's `typescript/consistent-type-definitions` enforces it); names are words, not abbreviations (`documentHandle`, `millimetersPerPixel`, `Rectangle`), with single letters only for `i` and the coordinates `x`, `y`, `z`. Names that come from the protocol schema or other specs keep their spelling (`DocHandle`, `PointRef`, `params`, `expr` in JSON). Every signature in this spec follows these rules.
- Styling follows [tailwind-css.md](../guidelines/tailwind-css.md): Tailwind utility classes only, written inline on the element; conditional classes with `clsx` imported from `clsx/lite` (never template literals or `+`); no `style` prop or attribute, with the one documented exception for runtime positions (3.12); no `px` values (Tailwind's rem scale; arbitrary values only where this spec names them, never in px); no CSS beyond `src/index.css` (2.16). Colors and the UI scale come from the role mapping in `src/theme/roles.ts` (2.16), and the two font faces from the theme variables of `src/index.css`, which components reach only through the inherited interface face, `font-mono` and `tabular-nums` (2.16.7); components never name a palette step or a font family themselves. Layout classes are not abstracted: repeated layout classes stay inline.

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
import type { Command, CommandResult, ErrorCode, FixAction } from '../generated/protocol'

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
  /** `ErrorInfo.detail`: technical text for Copy details (4.1), never shown as the message; null when absent. */
  readonly detail: string | null
  /** `ErrorInfo.actions` in order; empty when absent. */
  readonly actions: readonly FixAction[]
}

export type CoreClient = {
  /** Sends one command with its `u8` buffers (protocol.md 2.2; none unless given). Events in the response frame are dispatched before the promise settles. */
  request(command: Command, buffers?: readonly Uint8Array[]): Promise<Reply>
  onEvent(listener: (event: CoreEvent) => void): () => void
  onFatal(listener: (error: CoreFatalError) => void): () => void
  close(): void
}

export function createCoreClient(transport: Transport): CoreClient
```

Behavior:

1. Request ids start at 1 and increase by 1 per request; they never exceed `Number.MAX_SAFE_INTEGER`. Id 0 is never sent by the client (protocol.md 3.5).
2. `request` encodes a request frame with `encodeFrame({ protocol_version: PROTOCOL_VERSION, messages: [{ kind: 'request', id, command }] }, buffers)`, each buffer a `u8` buffer in the given order (`[]` without `buffers`), sends it, and decodes the response frame with `decodeFrame`.
3. It decodes every `event` message with `decodeEvent` (2.7) and calls each event listener synchronously in frame order, then settles the request: an `ok` outcome resolves with the result and the frame's buffers; an `error` outcome rejects with `CoreError`. Events with an unknown `type` are ignored (protocol.md 3.6).
4. A response whose id differs from the request id, or a frame that fails to decode, rejects the request with `CoreFatalError('The geometry core sent a response the app cannot read: <detail>')` and is reported to the fatal listeners: the client cannot trust the stream any more.
5. A listener that throws is reported with `console.error`; other listeners and the request are unaffected.

`src/core/queue.ts`:

```ts
/** What a task does, for a core that is stopped before it ends (4.4): a change to a document, or other work whose action reports its own drop. */
export type TaskKind = 'change' | 'other'
/** Runs tasks strictly one after another, in submission order. */
export type MutationQueue = {
  /** `kind` is `change` unless given. */
  run<Result>(task: () => Promise<Result>, kind?: TaskKind): Promise<Result>
  /** Rejects every task that has not started with `error`, without running it, and returns how many it rejected (4.4). */
  discard(error: Error): number
  readonly pending: number
  /** Whether the started task or a waiting one is a `change`. */
  readonly changing: boolean
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
/** Drops an input `same` as the one submitted last while `latest` is busy; `cancel` forgets that input. */
export function skipRepeats<Input>(
  latest: LatestOnly<Input>,
  same: (first: Input, second: Input) => boolean,
): LatestOnly<Input>
```

`createLatestOnly` semantics: when an in-flight send settles and a newer input is waiting, the settled result is discarded (neither callback runs) and the waiting input is sent; otherwise the matching callback runs. `flush()` resolves after the last callback has run, so a caller that awaits it never overtakes the last submitted input. A task that fails in `MutationQueue` rejects its own promise and does not block later tasks. `pending` counts the started and the waiting tasks; `discard` leaves the started task to settle with its own request, `pending` drops by the number it rejected, and tasks run after it run as usual. The open, new document, close, save, mark saved and export calls run as `other` tasks; every other task is a `change`.

Every document-mutating `CoreApi` call (apply, rename parameter, transaction, focus, undo, redo, open, new, close, end of drag, save, mark saved) runs through one `MutationQueue` per app, so the UI's order of edits is the core's order on both transports. Inspector previews and live rollback-marker drags (unlabeled `apply_edits` inside a transaction) go through a `LatestOnly` whose `send` enqueues on that queue, so they stay ordered with other mutations and coalesce while waiting. Sketch drags, sketch probes, evaluations, measurements (3.8), print analysis and the uses of 3.9 change nothing in the document; they use `LatestOnly` outside the queue. Whether the user waits on a request, for the still-working state, depends on its command and not on the queue (4.4): every queue task, sketch drags and sketch probes are waited on, while evaluations, measurements, print analysis and uses are background requests, which a user action that waits on them counts through `app.waitOn` (2.9).

### 2.6 Core API

`src/core/api.ts` wraps every command the M1 UI uses. Method names map one to one to protocol.md section 2.5 commands; argument and result shapes are the generated types.

```ts
import type {
  CatalogEntry, DocHandle, Edit, ElementDescriptor, ElementKind, ElementRef, ExportFormat, Diagnostic,
  Health, Pick, PointRef, PrinterProfile, Resolution, SketchParams, SketchSolution,
  Units, Dimension, Orientation, Measurement, MeasureTarget, FixRef, Use,
} from '../generated/protocol'
import type { AssetFile } from '../files/assets'

export type CoreApi = {
  engineInfo(): Promise<{ protocolVersion: number; engineVersion: string; formatVersion: number; kernel: string }>
  featureCatalog(): Promise<readonly CatalogEntry[]>
  listPrinterProfiles(): Promise<readonly PrinterProfile[]>
  newDocument(units?: Units): Promise<DocHandle>
  /** `open_document` with the asset files of the document's `assets/` folder (3.19); none unless given. */
  openDocument(text: string, assets?: readonly AssetFile[]): Promise<{ documentHandle: DocHandle; migratedFrom: number | null }>
  /** `save_document`: the canonical text, every asset the document references (sorted by name) and the revision the text describes. */
  saveDocument(documentHandle: DocHandle): Promise<{ text: string; assets: readonly AssetFile[]; revision: number }>
  markSaved(documentHandle: DocHandle, revision: number): Promise<void>
  closeDocument(documentHandle: DocHandle): Promise<void>
  applyEdits(documentHandle: DocHandle, label: string | null, edits: readonly Edit[]): Promise<number>
  /** `apply_fix`: resolves with the new revision. */
  applyFix(documentHandle: DocHandle, fix: FixRef): Promise<number>
  /** `set_document_units`: `kept` is how many numbers without a unit got the old unit. */
  setDocumentUnits(documentHandle: DocHandle, units: Units, keepSizes: boolean): Promise<{ revision: number; kept: number }>
  /** `list_uses`: what every feature uses, from the document alone. */
  listUses(documentHandle: DocHandle): Promise<readonly Use[]>
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
- `openDocument` sends `assets` as `[{name, data: i}]`, the bytes of asset `i` (in the given order) as the request's buffer `i` (`client.request(command, buffers)`), and `assets: []` without them. `saveDocument` copies each asset's `u8` buffer into a new `Uint8Array`, as `exportBodies` does, and keeps the result's order. `AssetFile` is the type of `src/files/assets.ts` (2.15).
- `measure` sends `{targets: [target]}`: the core takes exactly one target in M1 (protocol.md command 25).
- `applyFix`, `setDocumentUnits` and `listUses` map to protocol.md commands 32, 33 and 34. `applyFix` and `setDocumentUnits` change the document and run through the mutation queue; `listUses` changes nothing and runs outside it (2.5).
- The UI uses no other command in M1. `get_document`, `set_tessellation`, `put_printer_profile`, `import_step` and `export_python` are unused.

### 2.7 Core events

`src/core/events.ts` converts protocol events into UI events. Buffer indices are replaced by typed array views (no copies).

```ts
export type CoreEvent =
  | { readonly type: 'document_changed'; readonly documentHandle: DocHandle; readonly revision: number; readonly document: Document; readonly dirty: boolean; readonly undo: string | null; readonly redo: string | null; readonly transactionOpen: boolean; readonly transaction: string | null; readonly focus: string | null; readonly bareNumbers: number }
  | { readonly type: 'recomputed'; readonly documentHandle: DocHandle; readonly revision: number; readonly features: readonly FeatureStatus[]; readonly parameters: readonly ParameterValue[]; readonly diagnostics: readonly Diagnostic[]; readonly ghosts: Readonly<Record<number, Float32Array>> }
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

`print_health.overhang` maps body id to the `f32` overhang buffer of that body (`BodyPrint.overhang`). `recomputed.ghosts` maps the buffer index of every `Ghost` its statuses carry (each `Repair`'s `ghost` and each message's `cause.ghost`, protocol.md 2.4 and 3.4) to that `f32` buffer's view, so the statuses keep their generated types and a ghost's points are `ghosts[ghost.positions]`. `document_changed.bareNumbers` is `DocumentChanged.bare_numbers`. `decodeEvent` returns `null` for an unknown event type and throws `CoreFatalError` for a buffer index that is out of range or of the wrong element type.

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
  /** The hint about a refused pick, shown for 3 s (3.7, 3.16.1, 3.17), or null. */
  readonly pickHint: PickHint | null
  /** The feature type of the last feature command started in this session, which Repeat starts again (3.5); null before any. */
  readonly lastFeatureType: string | null
  /** The Keep sizes option of the units control (3.21): session state, on until the user turns it off. */
  readonly keepSizes: boolean
  /** The feature whose Problems row shows repair mode at rest (3.7, 3.9), or null; during a feature command repair mode follows the command. */
  readonly repair: RepairFocus | null
  /** What the last `show` action selected, with the cause's ghost it draws (4.2), or null; cleared when the selection changes. */
  readonly shown: ShownCause | null
}

/** The feature whose repairs repair mode shows; its current repair follows the active field (3.7). */
export type RepairFocus = { readonly feature: string }

/** A `show` action's feature and entity, and the ghost of the cause line it came from (absent for a message action). */
export type ShownCause = { readonly feature: string; readonly entity: string | null; readonly ghost: Ghost | null }

export type CoreState = {
  readonly status: 'loading' | 'ready' | 'failed'
  readonly engineVersion: string | null
  readonly fatal: string | null
  /** Mutating requests sent and not yet answered. */
  readonly pendingMutations: number
  /** True while the core has answered nothing for `STILL_WORKING_SECONDS` with a waited-on request waiting (4.4; background requests never count); false at start. */
  readonly stillWorking: boolean
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
  /** The key of this document session's recovery entry (3.19): a new `crypto.randomUUID()` for each document the app adopts (New, Open, a recovery Open), kept when a core restart reopens the document (4.4). */
  readonly recoveryId: string
  /** The asset files the core accepted when this session opened the document (3.19; none after an `asset_mismatch` retry), which a core restart gives it again (4.4); empty when none. */
  readonly assets: readonly AssetFile[]
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
  /** `bare_numbers` of the latest `document_changed`: expressions that are numbers without a unit (3.21). */
  readonly bareNumbers: number
  /** The ghost buffers of the latest `recomputed` (2.7), by buffer index. */
  readonly ghosts: Readonly<Record<number, Float32Array>>
  /** The latest `list_uses` result and the revision it describes, or null before the first (3.9). */
  readonly uses: { readonly revision: number; readonly uses: readonly Use[] } | null
}
export type MeshSummary = { readonly meshId: number; readonly boundingBox: BoundingBox; readonly faceCount: number; readonly edgeCount: number; readonly vertexCount: number }
export type FileReference = {
  readonly name: string
  readonly handle: FileHandle | null
  /** On the web, the folder the user chose that holds the file, through which the page reaches its `assets/` folder (3.19); null on the desktop and without one. */
  readonly folder: FileSystemDirectoryHandle | null
  /** `fileDigest` of the file's bytes as this session last read or wrote them, which Save compares to see a change made elsewhere (3.19); null without a handle or when unknown. */
  readonly digest: string | null
}

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
  /** Path of the selection field whose box the pointer is over, whose items the viewport draws hovered (3.7); absent or null for none. */
  readonly hoveredField?: string | null
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

/** A sketch record the command writes for holes placed on a face (3.7): one point entity per hole center, no constraints. `placement` is the face's placement in world coordinates (its `ElementDescriptor.placement`), on which the helper's points are drawn, since `params.plane.fallback` is in body coordinates. */
export type HelperSketch = { readonly id: string; readonly params: SketchParams; readonly placement: Placement; readonly applied: boolean }

/** The command state before one user change, with the label of the field the change touched. */
export type CommandStep = {
  readonly params: Readonly<Record<string, unknown>>
  readonly helper: HelperSketch | null
  readonly activeField: string | null
  readonly field: string
}

export type SketchSession = {
  /** Null while picking the plane for a new sketch, or for a sketch being re-attached. */
  readonly sketchId: string | null
  /** The sketch whose plane is being picked again (3.16.1, re-attaching), else null. */
  readonly reattaching: string | null
  readonly tool: SketchToolId | null
  readonly construction: boolean
  readonly showConstraints: boolean
  /** In-sketch selection (entities, points, constraints), separate from the app selection. */
  readonly selection: readonly SelectionItem[]
  readonly dragging: boolean
  /** Camera before enterSketch moved it, for leaving the sketch (3.16.11); null during plane picking. */
  readonly cameraBefore: Camera | null
  /** The `entity_used` hint of the last Delete (3.16.10), shown in the sketch hint for 8 s or until the next step prompt; absent or null for none. */
  readonly notice?: SketchNotice | null
}

/** A hint in place of the sketch's step prompt (3.16.10); each one is a new object. */
export type SketchNotice = { readonly message: string }

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
  /** Minor spacing in mm of the grid drawn now, mirrored by the viewport controller when it changes; null when no grid is drawn (3.12, 3.21). */
  readonly gridSpacing: number | null
}
export type SectionState = { readonly plane: 'XY' | 'XZ' | 'YZ'; readonly offset: string; readonly flipped: boolean }

export type PanelsState = {
  readonly browserOpen: boolean
  readonly timelineCollapsed: boolean
  readonly paletteOpen: boolean
  readonly side: 'parameters' | 'health' | 'problems' | 'keys' | null
  readonly inspectorPosition: { readonly x: number; readonly y: number } | null
}

/** A button of a notification; `editsDocument` marks a message's `apply` action and the 3.19 actions that save or reopen the document, which wait during a feature command (3.7, 2.9). */
export type NotificationAction = { readonly label: string; readonly run: () => void; readonly editsDocument?: boolean }

export type Notification = {
  readonly id: string
  readonly severity: 'info' | 'success' | 'warning' | 'error'
  readonly message: string
  readonly actions: readonly NotificationAction[]
  readonly createdAt: number
  /** The core's code for a core message shown verbatim (`CoreError.code`, `Diagnostic.code`), or the 4.3 id of a UI-owned message; shown as `data-code` (4.1). Absent when there is none. */
  readonly code?: string
  /** Feature ids a failure or loss notification, or a missing-assets warning (3.19), names; it is dismissed once none of them is in error and none of its `problems` remains (4.2). Absent for other notifications. */
  readonly features?: readonly string[]
  /** The cause line shown under the message, in `textMuted` (3.22); absent when there is none. */
  readonly cause?: string
  /** Keys `{feature} {code} {field}` of the new losses a loss notification reports (4.2); absent for other notifications. */
  readonly problems?: readonly string[]
}

/** A live region message; `id` changes with every announcement, so a repeated text is read again. */
export type Announcement = { readonly id: number; readonly message: string }

/** A hint about a pick the target refused: under the active command's selection field at `field`, or, when `field` is null, in the sketch hint while picking a plane (3.16.1) or in the print hint during Lay on face (3.17). */
export type PickHint = { readonly message: string; readonly field: string | null }

export type AppStore = StoreApi<AppState>
export const INITIAL_STATE: AppState
export function createAppStore(overrides?: Partial<AppState>): AppStore
```

`INITIAL_STATE`: core loading; no catalog, printers or document; context `model`; empty selection; no hover, command or sketch; print report null, overhang map on, orientation null, not picking a face; view perspective, named `iso`, no section, filter `{ body: false, face: true, edge: true, vertex: true }`, grid on, grid spacing null; browser open (`createAppStore` closes it when the window is narrower than 65rem at the stored UI scale, 3.2), timeline expanded, palette closed, no side panel; settings from `loadSettings()` (2.15); no notifications, announcement or pick hint; no feature command to repeat; Keep sizes on; no repair mode and nothing shown. A new `DocumentState` has `bareNumbers` 0, no ghosts and `uses` null; `bareNumbers` follows each `document_changed`, `ghosts` is replaced by each `recomputed`, and `uses` is kept until a newer result replaces it.

`announcement` is set by undo and redo (3.20) and by Copy details (4.1), and read by the polite live region. `pickHint` is set by `capturePick`, `createSketchOn` and `layOnFace` for a refused pick and cleared after 3 s, by a newer hint, or when its command ends. `lastFeatureType` is set by `startFeatureCommand` for every type but `sketch` (which starts plane picking), from any source.

`src/state/selectors.ts` exports at least `selectFeatureLabel(state, id)`, `selectActiveFeatureIds(state)`, `selectSketchDof(state)`, `selectHealthBadge(state)`, `selectProblems(state)` (the rows of 3.9, `problemRows`), `selectProblemCount(state)` (one per root and per document diagnostic, 3.9), `selectEmptyState(state)` (3.2), `selectOriginVisible(state, key)` (3.10), and `selectSketchToEdit(state)` and `selectSketchRadioTooltip(state)` (3.3); components read state only through selectors or `useAppState(selector)` so each panel re-renders only for the slice it shows.

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
  /** Restarts the core after a fatal error, or stops and restarts one that is still working, and reopens the settled recipe (4.4). */
  restartCore(): Promise<void>
  /** Counts `work`, background requests that a user action waits on before it goes on, as a waited-on request sent now, for the still-working state (4.4); settles as `work` does. */
  waitOn<Result>(work: Promise<Result>): Promise<Result>
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
/** `folder` is the web folder that holds the file, from a recent entry or the document it replaces (3.19); null unless given. */
export function openFile(app: App, file: OpenedFile, folder?: FileSystemDirectoryHandle | null): Promise<void>
export function openWithPicker(app: App): Promise<void>
export function openRecent(app: App, entry: RecentFile): Promise<void>
export function save(app: App): Promise<void>
export function saveAs(app: App): Promise<void>
export function applyEdits(app: App, label: string, edits: readonly Edit[]): Promise<boolean>
export function undo(app: App): Promise<void>
export function redo(app: App): Promise<void>
/** `set_document_units` with `keep_sizes` true when `keepSizes` is on and `bareNumbers` is above 0 (3.21). */
export function setUnits(app: App, units: Units): Promise<void>
export function setKeepSizes(app: App, keepSizes: boolean): void

// selection.ts
export function select(app: App, items: readonly SelectionItem[], mode: 'replace' | 'toggle'): void
export function clearSelection(app: App): void
export function setHover(app: App, target: HoverTarget | null): void
export function pickAt(app: App, sample: PickResult, modifiers: { toggle: boolean }, pointer?: PointerRay): Promise<void>

// command.ts
export function startFeatureCommand(app: App, featureType: string): Promise<void>
export function editFeature(app: App, featureId: string): Promise<void>
/** `typing` marks a text edit of an expression field: only the first since the field gained focus is a history step (3.7). */
export function setCommandField(app: App, path: string, value: unknown, options?: { typing?: boolean }): void
export function setActiveField(app: App, path: string | null): void
/** The selection field whose box the pointer is over, or null (3.7: the viewport highlights its items). */
export function setHoveredField(app: App, path: string | null): void
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
/** Picks a new plane or face for a sketch whose face is lost (3.16.1, re-attaching). */
export function reattachSketch(app: App, sketchId: string): Promise<void>
/** "Show '{sketch}'" of `needs_closed_shape` (2.10): enters the sketch and frames all its open ends, as the sketch problems popover's Show frames one finding (3.16.8). */
export function showOpenEnds(app: App, sketchId: string): Promise<void>
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
/** Asks the core for `list_uses` when `document.uses` is older than the document's revision (3.9); resolves when it is current. */
export function refreshUses(app: App): Promise<void>

// fixes.ts
/** Runs one action of a message or of its cause line (4.2): `apply` through `apply_fix`, `pick`, `show` and `undo` as host actions. `place` is where it is offered: the command's own banner (and its repair mode's markers), whose `apply` joins the transaction, or a panel or a notification, whose `apply` waits during a feature command (3.7). `cause` is the cause line the action belongs to, whose ghost a `show` draws. */
export function runFixAction(app: App, action: FixAction, place: 'banner' | 'panel' | 'notification', cause?: Cause): Promise<void>
/** Copies the code, the message, the cause and the detail of a message or core error for reports (4.1). */
export function copyDetails(app: App, message: { readonly code: string; readonly message: string; readonly cause?: Cause | null; readonly detail?: string | null }): Promise<void>
/** "Show what '{label}' lost": opens repair mode for the feature's first repair and frames it (3.7). */
export function showLost(app: App, featureId: string): Promise<void>

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
/** Ends the Lay on face pick without a face (Esc ladder step 5). */
export function cancelLayOnFace(app: App): void
/** The face clicked during Lay on face: a flat face of a visible part body sets `face_down` and ends the pick; another face shows `pick_flat_face`. Returns whether it was taken. */
export function layOnFace(app: App, item: SelectionItem): boolean
/** Sets the session print orientation and ends the face pick; null prints as modeled. */
export function setPrintOrientation(app: App, orientation: Orientation | null): void

// export.ts
export function exportModel(app: App, request: ExportRequest): Promise<void>

// notifications.ts
export function notify(app: App, notification: Omit<Notification, 'id' | 'createdAt'>): string
export function dismiss(app: App, id: string): void
```

Edits that send a whole record (`update_feature` of a rename or suppress, `update_body`, `update_parameter`) are built when their turn in the mutation queue comes, from the record as the edits before them left it, so one queued behind another change of the same record changes only its own property (a rename followed at once by a suppress keeps the new name); edits never follow their document into another one that replaced it meanwhile, while a reopen that keeps the document session (Choose folder, 3.19) is not another one: an edit waiting behind it goes to the reopened document. `apply_fix` needs no such care: the core builds a fix's edits from the current records (engine.md section 3.22). Edits refused during a feature command (3.7) refuse in the action itself, with `finish_command_first` as an info notification: `save`, `saveAs`, `setUnits`, `setPrinter`, the body and timeline edits, `apply` actions that do not belong to the command's banner (`runFixAction`, 4.2), and the notification actions of 3.19 that save or reopen the shown document ("Choose folder" of `assets_folder_needed`, `save_folder_needed` and `assets_wrong_folder`, and "Overwrite {file}" of `save_conflict`). Those notification actions have `editsDocument` true, so their buttons wait during the command as a message's `apply` does (3.7) and the notification stays.

`command.ts` also exports the helpers the inspector, the typing keys and the Parameters panel use: `setFieldStatus` (an expression field's error and pending state), `noteFieldFocused` (an expression field gained focus, so its next text edit is a new history step), `chooseNewBody` (the Choose menu's "New body"), `reverseAndUse` (the fix "Reverse and use {text}" as one history step), `cancelCommandImplicitly` (the implicit cancel with `command_canceled` and Restore), `applyEditsInCommand` and `runInCommand` (edits that join the open transaction with the preview `pending` meanwhile), `commandValueField` (the value field of typing values) and `commandStartParams` (the start params `changedMore` compares with). `startFeatureCommand` and `editFeature` take an internal third argument that Restore uses to reopen a saved command.

`ExportRequest` is `{ format: '3mf' | 'stl' | 'obj' | 'step'; scope: 'visible' | 'selected' }`. `RollbackDrag` is `{ move(position: number): void; drop(): Promise<void>; cancel(): Promise<void> }`, the live marker drag of 3.9 from gap `start`. `SketchChange`, `SketchCommitResult`, `PickResult` and `PointerRay` are defined in 2.14 and 2.11. Every action that changes the document goes through `app.mutations`.

### 2.10 Commands, shortcuts, toolbars and palette search

`src/commands/registry.ts`:

```ts
export type CommandCategory = 'File' | 'Edit' | 'View' | 'Create' | 'Modify' | 'Pattern' | 'Construct' | 'Sketch' | 'Constrain' | 'Print' | 'Settings'

export type Availability =
  | { readonly state: 'enabled' }
  /** `code` is the 4.3 message id of the reason; the notification a click or shortcut shows carries it as its `code` (`data-code`, 3.22), and `action` as its one action. */
  | { readonly state: 'disabled'; readonly reason: string; readonly code?: MessageId; readonly action?: { readonly label: string; readonly run: (app: App) => void } }
  | { readonly state: 'hidden' }

export type InvocationSource = 'toolbar' | 'palette' | 'shortcut' | 'menu' | 'inspector'

/** A search word; a tagged word is another app's name for the command (2.10, `COMMAND_KEYWORDS`). */
export type Keyword = string | { readonly word: string; readonly app: 'Fusion' | 'FreeCAD' }

export type CommandDefinition = {
  /** Unique id, for example 'file.save' or 'feature.extrude'. */
  readonly id: string
  readonly title: string
  /** The title as shown now when it names the state, such as "Repeat Fillet" (3.5); the palette and menus show it. Absent means `title`. */
  label?(app: App): string
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
  /** Contexts in which the shortcuts are active when that is fewer than `contexts` (3.5); absent means `contexts`. */
  readonly shortcutContexts?: readonly AppContext[]
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
  /** Runs the command if enabled; a disabled command shows its reason as an info notification, with the availability's `action` when it has one. */
  run(app: App, id: string, source: InvocationSource): Promise<void>
  /** The command bound to a chord in a context, considering host, shortcut contexts and availability 'hidden'. */
  forChord(app: App, chord: string, context: AppContext): CommandDefinition | undefined
  /** Clashes of chords within each command's shortcut contexts. */
  conflicts(platform: Platform): readonly ShortcutConflict[]
  subscribe(listener: () => void): () => void
}
export function createCommandRegistry(): CommandRegistry
/** Whether a command's shortcuts are active in a context: `shortcutContexts`, else `contexts`. */
export function shortcutsActiveIn(definition: CommandDefinition, context: AppContext): boolean
```

`src/commands/shortcuts.ts`:

```ts
/** A parsed chord. `key` is normalized (2.10). */
export type Chord = { readonly primaryModifier: boolean; readonly control: boolean; readonly alt: boolean; readonly shift: boolean; readonly key: string }
export function parseShortcut(shortcut: string): Chord
export function chordToString(chord: Chord): string
export function formatShortcut(shortcut: string, platform: Platform): string
export function chordFromEvent(event: KeyboardEvent, platform: Platform): Chord | null
/** A shortcut as `aria-keyshortcuts` writes it: Control, Alt, Shift and Meta (`Mod` is Meta on macOS, Control elsewhere), then the key, joined by "+". */
export function ariaKeyShortcut(shortcut: string, platform: Platform): string
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

`src/commands/features.ts` registers one command per catalog entry: id `feature.<type>`, title the manifest label, category from the manifest category (`create` Create, `modify` Modify, `pattern` Pattern, `construct` Construct, `sketch` Create), icon from `FEATURE_ICONS[manifest.icon]` (fallback `Shapes`), keywords from `COMMAND_KEYWORDS['feature.' + type]`, description from `FEATURE_DESCRIPTIONS['feature.' + type]`, shortcuts and their `shortcutContexts` from `FEATURE_SHORTCUTS[type]` (3.5), contexts `model`, `sketch` and `print` (running it from Sketch or Print switches to Model first, 3.3). The `sketch` type's command runs `startNewSketch` instead of the inspector. The `import` type gets no command (STEP import UI is later). Its timeline item can be selected, renamed, suppressed and deleted; Edit is disabled with `import_not_editable` (4.3).

Feature command availability (`FEATURE_REQUIREMENTS` in `src/commands/features.ts`) is computed from the store, so a tool that has nothing to act on says what is missing before it is clicked; editing an existing feature is never affected:

- `extrude`, `revolve`: disabled when no active sketch (not suppressed, not rolled back) has a region in its latest solution (`document.sketches`); the sketch being edited in a sketch session counts. When an active sketch without a region has open ends (`openEnds`, 2.14, at least 1), the reason is `needs_closed_shape` for the one with the fewest open ends, the latest in timeline order among equals, with the action "Show '{sketch}'", which enters that sketch and frames its open ends (the framing of the sketch problems popover's Show, 3.16.8, over the points of all its `open_endpoint` findings); otherwise it is `needs_sketch_region`.
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

/** The canvas size and the free area inside it (3.2), in CSS pixels from the canvas's top-left corner. */
export type ViewWindow = {
  readonly width: number
  readonly height: number
  readonly free: { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
}
/** `projectionMatrix` for the free area's aspect, followed by the clip-space scale and shift that put the free area where it lies on the canvas: an off-center frustum. */
export function windowProjection(camera: Camera, view: ViewWindow, near: number, far: number): Float32Array
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
- `cameraAfterSketch`: when the angle between `forward(current)` and `normal` or `-normal` is more than 10 degrees, null (the user already turned the view, so it stays). Otherwise `before` when it is not null and its forward vector is itself more than 10 degrees from `normal` and `-normal`; otherwise `current` with the `iso` rotation (`standardViewRotation('iso')`), keeping target, distance, projection and orthographic height, or with the `front` rotation when the iso view is itself within 10 degrees of `normal` or `-normal` (a plane facing the iso direction). It never returns a camera looking straight at the sketch plane, so the next feature's preview is seen in depth.
- The camera's window is the free area of 3.2, not the canvas. Every function above that takes an `aspect` or a viewport height in pixels gets the free area's (`free.width / free.height`, `free.height`), and normalized device coordinates (`cursorNormalized`) run from -1 to 1 across the free area and beyond that range outside it: a canvas point (x, y) has the free-area coordinates `(2 (x - free.x) / free.width - 1, 1 - 2 (y - free.y) / free.height)`. So the visible height spans the free area's height, fit fills 80 percent of the free area's limiting dimension around its center, pan moves `visibleHeightAtTarget / free.height` millimeters per pixel, and `gridSpacing` and `drawnGridSpacing` measure the free area. The same holds for every other mapping between pixels and the scene: the scene's `frame` and `screenRectangle` take the `ViewWindow`, and its pixel-sized quantities (the sketch chord tolerance, pick sizes, the grid) use `free.height`; the sketch session's pixel mappings (`pixelToSketch`, `sketchToPixel` and `millimetersPerPixelAt` in `src/sketch/placement.ts`) take the `ViewWindow` from `app.viewport()?.viewWindow()` and work in free-area device coordinates, so drawing, snapping and dimension labels stay under the cursor. `windowProjection` multiplies `projectionMatrix(camera, free.width / free.height, near, far)` on the left by the clip-space matrix that scales x by `free.width / width` and y by `free.height / height` and adds `(2 free.x + free.width) / width - 1` and `1 - (2 free.y + free.height) / height` times w to them: the target projects to the free area's center, and the canvas around the free area shows more of the same projection, at the same scale. A free area with a side below 1 CSS pixel (not measured yet) counts as the whole canvas, which gives back `projectionMatrix` with the canvas's aspect. Picking, the ID pass and `screenRectangle` stay in canvas pixels.
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
  /** Lines: width in CSS pixels; points: size in CSS pixels; both already times the UI scale, so the backend multiplies them by the pixel ratio only (its own emphasis and dash lengths, at 100 percent, by the UI scale too). */
  readonly size: number
  /** Lines only: dashed (construction geometry). */
  readonly dashed: boolean
  readonly shading: 'matcap' | 'flat'
  readonly depthTest: boolean
  readonly clipped: boolean
  /** First pick id of this draw, or null when not pickable. Element i has pick id pickBase + i. */
  readonly pickBase: number | null
  /** Pick footprint in CSS pixels for lines and points in the ID pass, already times the UI scale. */
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
  /** The viewport's one flat background color (3.12 pass 1). */
  readonly background: Rgb
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
  /** Screen rectangle (CSS pixels) covering selection items, or null: a body's bounding box, a flat face's vertices in its body's model mesh (those in its plane facing its way), a straight edge's two ends, other faces, edges and vertices by their centroid, sketch items by their points (`src/viewport/element-extent.ts` for faces and edges). */
  screenRectangle(items: readonly SelectionItem[], camera: Camera, view: ViewWindow): Rectangle | null
  /** The world points `screenRectangle` projects for the same items: what framing them shows (4.2 Show). */
  worldPoints(items: readonly SelectionItem[]): Vector3[]
  /** `frame.projection` is `windowProjection(camera, view, near, far)` (2.11). */
  frame(state: AppState, camera: Camera, view: ViewWindow & { readonly pixelRatio: number }): RenderFrame
  restore(): void
  /** Model elements drawn selected besides selected bodies: resolved references and feature highlighting (3.6). */
  setElementHighlights(highlights: ElementHighlights): void
  /** Edges drawn hovered in place of a hovered face that stands for its edges (3.7 faces for edges), or null. */
  setHoverEdges(hover: { faceKey: string; body: string; meshId: number; edges: readonly number[] } | null): void
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
  /** The world box of `scene.worldPoints(items)`, or null when none of the items is drawn; a `show` action frames it (4.2). */
  itemBounds(items: readonly SelectionItem[]): BoundingBox | null
  screenRectangle(items: readonly SelectionItem[]): Rectangle | null
  /** The canvas size and the free area last measured (3.2). */
  viewWindow(): ViewWindow
  requestRender(): void
}
export type ViewportController = ViewportHandle & { dispose(): void }
/** `overlay` is the overlay layer of 3.12 whose positioned children the controller moves each frame; `freeArea` is the `viewport` element, the free area of 3.2. */
export function createViewportController(options: { app: App; canvas: HTMLCanvasElement; overlay: HTMLElement; freeArea: HTMLElement; backend: RenderBackend; scene: Scene }): ViewportController
```

The controller measures the free area as `freeArea`'s bounding rectangle minus the canvas's, after layout, on a `ResizeObserver` of both elements and on window resize, and builds every frame's projection with `windowProjection`. A change of the free area (a docked island opened, closed or collapsed, a window resize) renders a frame and moves the anchored overlay elements. While the camera is still the one the last `fit` or `zoomToBounds` set (no orbit, pan, zoom, view change or other camera change since), the change frames the same bounds again with the same margin, at once, without animation or history, so a model fitted by its width keeps its ends out from under an island that opens. Otherwise the camera is kept: its target, rotation and visible height stay, so the model moves to the new free area's center, keeps its size when only the width changed, and grows or shrinks with the free area's height. A shape fitted by its height gives the same picture either way.

While the inspector (3.7) is open, `fit`, `zoomToBounds` and Look at (3.14) frame into the part of the free area beside it, since the inspector is an island too (ADR 0030): the part right of the inspector's right edge when the inspector's center lies in the free area's left half, else the part left of its left edge, or the whole free area when that part is narrower than half of it. Only the part of the card inside the free area counts, so a card that lies under an island frames the whole free area. They compute the camera for that part's aspect (its height is the free area's), then `pan` it by the offset from the free area's center to the part's center, so the bounds' center (for Look at, its target point) projects to the part's center. Opening, moving or closing the inspector moves no camera.

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
/** The plane the grid lies on: XY in Model context and while a plane is picked, the sketch plane while a sketch is edited; null in Print and while the grid is off. */
export function gridPlacement(state: AppState): Placement | null
/** The minor spacing of the grid drawn for the state and camera (`gridSpacing` at the camera target), or null when none is drawn. */
export function drawnGridSpacing(state: AppState, camera: Camera, viewportHeightPixels: number): number | null
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

`command` is the registry command an item runs (`edit.edit_feature` for "Edit {label}", which opens the named feature), or `command.confirm` and `command.cancel` for the OK and Cancel of an active feature command, whose shortcuts are `formatShortcut('Enter')` and `formatShortcut('Escape')`. `src/viewport/ViewportMenu.tsx` renders the items as a `Menu` (`data-testid="viewport-menu"`) anchored to a point-sized element at the request position (`positionElement`, 3.12; the anchor is a child of `main` outside the free-area layer, so its containing block is the app root and its CSS pixels are canvas pixels, 3.2) and gives focus back to the 3D view when it closes. It opens on the `UiRequest` `{kind: 'viewport_menu', x, y}` (CSS pixels from the canvas's top-left corner), which the controller sends after a right click and the viewport after Shift+F10 or the ContextMenu key; a request whose items are all left out opens nothing.

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
  /** The coordinate an X, Y or Z field holds (its letter, and whether it runs along that world axis), else null. */
  readonly axis: { readonly letter: 'X' | 'Y' | 'Z'; readonly world: boolean } | null
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
  /** The starting values the params are compared with for `changedMore`: `defaultParams` without the reuse step. Without it nothing counts as changed (callers that only need the fields omit it). Tagged unions of the start params are switched to the variants the params use, by `setParam`'s rules, before comparing. */
  readonly startParams?: Readonly<Record<string, unknown>>
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
| string `enum`, or `oneOf` of `const` strings, or the tag property (`kind`) of a tagged union | `options` set | `choice`, `segmented` for at most 4 options (drawn as a select when its segments do not fit the field, 3.7), else `select`, with the hint's labels in the hint's order |
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
- `axis`: for an `expression` field with unit `length` whose path ends in `[0]`, `[1]` or `[2]` and whose label ends in " X", " Y" or " Z" with the matching letter (Move X to Move Z of Move body, Position X to Position Z of the primitives), `{letter, world}`; null for every other field. `world` is false when the feature's params have a top-level `plane` (a `PlaneRef`) other than the origin XY plane (`{"kind": "origin", "plane": "XY"}`), because the coordinates then run along that plane's own axes (features.md, the primitives' `position`), and true otherwise (Move body's translation runs along world X, Y and Z).
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

`src/inspector/selection-values.ts` builds param values from selection items, exactly in the shapes of features.md 2.5. A face's, edge's or vertex's `Reference` is the full one `pick_reference` returns (anchors and merged names, engine.md section 3.9), which is what a pick stores; the short references of `list_elements` are never stored (3.7, faces for edges):

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
| `axis` | origin axes, sketch lines, edges whose `curve` is `line`, faces whose `surface` is `cylinder`, `cone`, `sphere` or `torus` | `{"kind": "origin", "axis": "Z"}`, `{"kind": "sketch_line", "sketch": id, "line": id}`, `{"kind": "edge", "body": id, "edge": <Reference>, "direction": <axis_direction>}` or `{"kind": "face", "body": id, "face": <Reference>, "direction": <axis_direction>}`, with `direction` the descriptor's `axis_direction` (body coordinates, features.md 2.5), left out when the descriptor has none |

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
  /** A `data-testid` for the input itself, such as `entity-size` (3.8). */
  readonly testId?: string
  readonly label: string
  readonly value: string
  /** A field unit, or 'any' for parameter expressions (every dimension accepted). */
  readonly unit: FieldUnit | 'any'
  readonly optional?: boolean
  readonly autoFocus?: boolean
  readonly selectOnFocus?: boolean
  /** Keeps the label for assistive technology only. */
  readonly hideLabel?: boolean
  /** A message about the value from elsewhere (a feature status), shown while the text itself is not invalid. */
  readonly error?: string | null
  /** The largest value that works (3.7), shown while the text is not invalid as `field_limit` under `field-error` (`field-limit`, in the input's `aria-describedby`), with the `warningMark` border in place of `errorMark`; or null. */
  readonly limit?: string | null
  /** The field's hover help (3.7), or null. */
  readonly help?: string | null
  /** Offers "Add parameter {name}" (3.7); the command and selection inspectors set it. */
  readonly offerParameter?: boolean
  /** Name completion (below), default true; the sketch dimension editor passes false and stays a plain text input. */
  readonly completion?: boolean
  /** Controls at the end of the input's row, such as the inline Reverse toggle (3.7). */
  readonly trailing?: React.ReactNode
  /** The coordinate the field holds (2.12): its `AxisLetter` is drawn inside the input's start and the input gets `pl-6` (3.7). */
  readonly axis?: InspectorField['axis']
  /** Sets the text as code (2.16.7): `font-mono` and `[font-variant-ligatures:no-contextual]` in place of `tabular-nums`. Only the Parameters panel's expression field sets it (3.11); default false. */
  readonly code?: boolean
  /** Called with the text on every edit, and again when the state of the text changes; a text set from outside (an undo, a fix) is reported when it renders. */
  readonly onChange: (value: string, state: ExpressionState) => void
  readonly onCommit?: (value: string, state: ExpressionState) => void
  readonly onCancel?: () => void
  readonly onFocus?: () => void
  readonly onBlur?: (value: string, state: ExpressionState) => void
}
export function ExpressionInput(props: ExpressionInputProps): React.JSX.Element
export function useExpression(text: string, unit: FieldUnit | 'any', label: string): ExpressionState
```

`useExpression` evaluates through `app.core.evaluate` with `createLatestOnly`, converts the result with `convertForField` (for `any`, the value and dimension are kept as returned) (results for an older text never show), re-evaluates when the document revision changes, and returns `pending` while waiting. Empty text is `empty`: valid for optional fields, otherwise `invalid` with the message `{label} needs a value.`

Pressing Enter in an `invalid` state with a non-null `range` keeps the input focused and selects that range (`setSelectionRange(range.start, range.end)`), so the user can retype the offending part; the message stays in `field-error`.

`onChange` reports a state of the text no later than the commit that renders it (a typed text from its input event, a result or a text set from outside from a layout effect), before the browser can deliver further input, so no Enter, click or key reaches the app between a shown result and its report: an owner that counts the field as pending until told otherwise, as the command inspector's confirm does (3.7), finds the field settled whenever its result shows, and `onCommit` comes after the report of the state it commits.

Name completion, in every `ExpressionInput` (inspector fields, the selection inspector's dimension and size fields, the Parameters panel and the section offset; the sketch draw inputs and the dimension editor are plain text inputs and have none):

- Names: the document's parameters, shown as "{name} = {value}" (the value formatted as in 3.11), and the `printer.*` names of print.md (`printer.fit.press`, `printer.fit.snug`, `printer.fit.slip`, `printer.fit.loose`, `printer.layer_height`, and `printer.nozzle` for an FDM profile), shown as the name.
- Opening: while the caret ends an identifier (letters, digits, `_` and `.`, starting with a letter or `_`) that some name matches with a non-null `fuzzyScore`, a listbox under the input (`data-testid="expression-suggestions"`, options `role="option"` in the field's own face, so a name looks the same in the list and in the field: `tabular-nums`, or `font-mono` in a `code` field, 2.16.7) lists up to 8 such names, best score first, then by name. The listbox is a popover in the top layer (`popover="manual"`, `w-max` from `min-w-48` to `max-w-80`), placed at runtime like the menus of 3.2: under the input, or above it when only that side has room, so neither a scrolling side panel nor the inspector's body cuts it off. It follows the input when a panel scrolls and when the input moves without a scroll or a resize, as when the inspector's banner appears above it while the user types (`useAnchoredPosition` with `followMoves`, which checks the input's box once per animation frame while the list is open). It never opens for an identifier that directly follows a number (`10 mm`, `1 in`), that is a unit word, `pi` or a function name of expressions.md, or that equals a name exactly. A character that cannot be part of a name closes it.
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
  /** `message`: the probe solution's first `sketch_conflict` or `redundant_constraints` message (sketch.md section 4.2), null without one. */
  | { readonly outcome: 'over_constrained'; readonly constraint: string; readonly conflicting: readonly string[]; readonly redundant: boolean; readonly message: string | null }
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

// src/sketch/issues.ts
/** The outline summary of the header (3.16.8) and of `profile_problems` (4.3): "1 open end", "{n} open ends", "1 outline problem" or "{n} outline problems"; null without profile findings. */
export function outlineProblems(solution: SketchSolution): string | null
/** The open ends of the solution's outline: an `open_endpoint` finding counts 1, or 2 when it has `connect` (it stands for two ends, sketch.md section 4.3). */
export function openEnds(solution: SketchSolution): number
/** The message of `solution.messages` for profile finding `index` (its `field` is `/profile_findings/{index}`), or null. */
export function findingMessage(solution: SketchSolution, index: number): Diagnostic | null
```

`outlineProblems` names open ends when every finding is `open_endpoint`, with n `openEnds(solution)`; otherwise outline problems, with n the number of findings. A finding with `connect` is one problem but two ends, so the gap of newcomer error corpus NC01 reads "2 open ends", as the Extrude reason of 2.10 counts it.

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
export class FileServiceError extends Error {
  readonly reason: 'permission' | 'not_found' | 'io'
  /** The entry of the document's folder a failure is about, `assets` or `assets/{name}`, for Copy details (3.19); null for the file itself. */
  readonly entry: string | null
  /** `entry` is null unless given. */
  constructor(reason: 'permission' | 'not_found' | 'io', message: string, options?: ErrorOptions & { readonly entry?: string })
}
/** A document's file and, on the web, the folder the user chose that holds it (3.19); the desktop host finds the folder from the grant. */
export type DocumentPlace = { readonly handle: FileHandle; readonly folder: FileSystemDirectoryHandle | null }
/** What `readAssets` read, in the order asked, and its first failure (null when none). */
export type AssetReading = { readonly assets: readonly AssetFile[]; readonly failure: FileServiceError | null }
/** The answer to "choose the folder that holds this file" (3.19): `other` is a folder that does not hold it directly, by its name. */
export type FolderChoice =
  | { readonly kind: 'chosen'; readonly folder: FileSystemDirectoryHandle }
  | { readonly kind: 'canceled' }
  | { readonly kind: 'other'; readonly name: string }
export type FileService = {
  readonly fileSystemAccess: boolean
  /** Null when the user cancels. */
  pickToOpen(): Promise<OpenedFile | null>
  /** `mode` is the permission the web asks for: `read` unless given; Save's change check asks `readwrite` (3.19). `folder` is the web folder that holds the file, whose permission is asked for first; null unless given. */
  readHandle(handle: FileHandle, mode?: 'read' | 'readwrite', folder?: FileSystemDirectoryHandle | null): Promise<OpenedFile>
  /** Asks where to save. Null when the user cancels. Without the File System Access API it returns a download target at once. */
  chooseSaveTarget(suggestedName: string, kind: FileKind): Promise<SaveTarget | null>
  /** Writes the bytes to the target (a download for download targets). */
  write(target: SaveTarget, kind: FileKind, bytes: Uint8Array): Promise<void>
  /** Whether `readAssets` and `writeAssets` reach the `assets/` folder next to the file of `place`. */
  reachesFolder(place: DocumentPlace): boolean
  /** Asks for the folder that holds the file (the web; the desktop never needs it). */
  chooseFolder(handle: FileHandle): Promise<FolderChoice>
  /** Reads the named assets from the `assets/` folder next to the file, leaving out those that are not there. */
  readAssets(place: DocumentPlace, names: readonly string[]): Promise<AssetReading>
  /** Writes the assets, in order, to the `assets/` folder next to the file with the entry check of file-format.md 3.11 step 1; throws at the first failure. */
  writeAssets(place: DocumentPlace, assets: readonly AssetFile[]): Promise<void>
}
export function createWebFileService(environment: Window & typeof globalThis): FileService
export const FILE_TYPES: Readonly<Record<FileKind, { readonly description: string; readonly mediaType: string; readonly extensions: readonly string[] }>>
/** Whether a web folder holds a file directly: `folder.resolve(handle)` is `[handle.name]`; false for a desktop handle. */
export function folderHolds(folder: FileSystemDirectoryHandle, handle: FileHandle): Promise<boolean>
```

`src/files/assets.ts` and `src/files/digest.ts`:

```ts
/** A file of a document's `assets/` folder: its asset name and its bytes (file-format.md 3.11). */
export type AssetFile = { readonly name: string; readonly bytes: Uint8Array }
/** An asset's file name (file-format.md 3.11): 64 lowercase hex digits, a dot and a lowercase extension. */
export const ASSET_NAME: RegExp
/** The asset names among the string values anywhere in a document's JSON text, each once, sorted; none when the text is not JSON. */
export function referencedAssetNames(text: string): readonly string[]

/** The SHA-256 of the bytes (Web Crypto's `crypto.subtle.digest`) as 64 lowercase hex digits; rejects where Web Crypto cannot digest (no `crypto.subtle` outside a secure context). */
export function fileDigest(bytes: Uint8Array): Promise<string>
```

- `ASSET_NAME` is `/^[0-9a-f]{64}\.[a-z0-9]+$/`, the pattern of python-api.md 3.1. `referencedAssetNames` parses the text with `JSON.parse` and collects every string value, at any depth, that matches it (object keys are not values); a feature's `asset` param is one, and the core ignores a supplied asset that no feature references (engine.md 3.14), so a matching string elsewhere only costs a read.
- `fileDigest` digests exactly the bytes of the view (not the rest of its buffer). Where it rejects, the UI takes the digest as null and logs `console.warn('Partsmith could not compute the digest of {file}, so Save does not check it for changes made elsewhere.', error)`, and Open and Save go on without the change check (3.19).

`FILE_TYPES`: `ptsm` "Partsmith model", `application/json`, `.ptsm`; `3mf` "3MF model", `model/3mf`, `.3mf`; `stl` "STL mesh", `model/stl`, `.stl`; `obj` "OBJ mesh", `model/obj`, `.obj`; `step` "STEP model", `model/step`, `.step` and `.stp`.

- `fileSystemAccess` is true when `showOpenFilePicker` and `showSaveFilePicker` are functions.
- `pickToOpen` uses `showOpenFilePicker({ types: [ptsm], excludeAcceptAllOption: false, multiple: false })`; without the API it clicks a hidden `<input type="file" accept=".ptsm">` and resolves on `change` (null when the input reports no file). An `AbortError` resolves null.
- `readHandle(handle, mode, folder)` first, when `folder` is given, checks the folder's `queryPermission({ mode: 'readwrite' })` and calls its `requestPermission({ mode: 'readwrite' })` when it is `prompt`, so the one prompt of the user's click is the folder's, which in Chromium also covers the files in it (open issue 39); a refusal or a failure of that request is ignored here (`readAssets` and `writeAssets` report it). Then it checks the file's `queryPermission({ mode })` (`read` unless given), calls `requestPermission({ mode })` when it is `prompt`, and throws `FileServiceError('permission')` when not granted, then reads `getFile()`. A `NotFoundError` throws `FileServiceError('not_found')`.
- `chooseSaveTarget` uses `showSaveFilePicker({ suggestedName, types: [FILE_TYPES[kind]] })` and returns `{ kind: 'handle', name: handle.name, handle }`; an `AbortError` returns null. Without the API it returns `{ kind: 'download', name: suggestedName }`.
- `write` to a handle checks `queryPermission({ mode: 'readwrite' })`, calls `requestPermission` when it is `prompt`, and throws `FileServiceError('permission')` when not granted; then `createWritable()`, `write(bytes)`, `close()`. A `NotFoundError` throws `FileServiceError('not_found')`; anything else `FileServiceError('io')` with the original message. `write` to a download target creates a `Blob` of the bytes with the kind's media type, an object URL and an `<a download="{name}">`, clicks it, and revokes the URL after the click.
- Every `FileServiceError` of `pickToOpen`, `readHandle`, `chooseSaveTarget` and `write` has `entry` null.
- `reachesFolder(place)` is true when `place.handle` is a web handle and `place.folder` is not null.
- `chooseFolder(handle)` calls `showDirectoryPicker({ mode: 'readwrite', startIn: handle })` (every browser with the two file pickers has it), so the dialog opens in the file's folder and one permission covers reading and writing it; an `AbortError` returns `canceled`, and any other error throws `FileServiceError('io')` with its message. It returns `chosen` when `folderHolds(folder, handle)`, else `other` with `folder.name`. A desktop handle, and a browser without `showDirectoryPicker`, throws the `permission` error of `readHandle`.
- `readAssets(place, names)` returns `{assets: [], failure: null}` when it does not reach the folder. Otherwise it checks the folder's `readwrite` permission as `write` does for a file (a refusal is a `permission` failure with `entry` `assets`), then `folder.getDirectoryHandle('assets')`: a `NotFoundError` reads nothing, and a `TypeMismatchError` (a file in the folder's place) is an `io` failure with `entry` `assets`. For each name in order it calls `getFileHandle(name)` and `getFile()`: a `NotFoundError` leaves the asset out, and any other error (a `TypeMismatchError` for a folder of that name) is a failure with `entry` `assets/{name}` that leaves it out while the reading goes on. The first failure is returned.
- `writeAssets(place, assets)` throws `FileServiceError('io', 'Partsmith has no access to the folder of {name}.')` with `entry` `assets`, writing nothing, when it does not reach the folder. Otherwise it checks the folder's `readwrite` permission as `readAssets` does and creates `assets` with `getDirectoryHandle('assets', { create: true })` (a `TypeMismatchError` is an `io` failure for `assets`). Then it writes each asset in order after the entry check of file-format.md 3.11, as far as the API can see:
  1. `getFileHandle(name)`. A `TypeMismatchError` (a folder of that name) throws an `io` failure for `assets/{name}`, and the folder stays.
  2. A file whose `getFile()` has the asset's size is read (`arrayBuffer()`) and kept, not written, when its bytes equal the asset's; a file of another size is not read.
  3. Any other file is replaced whole: `createWritable()`, `write(bytes)`, `close()`, with `abort()` on a failure, so the file keeps its old bytes.
  4. A missing entry (`NotFoundError`) is created with `getFileHandle(name, { create: true })` and written the same way, and removed with `removeEntry(name)` when its write fails, so no empty file is left under an asset's name.

  The first failure throws `FileServiceError` with the reason `write` gives it and `entry` `assets` or `assets/{name}`, and nothing after it is written. The page can neither tell a symbolic link from what it points to nor flush a file (file-format.md 3.11): an entry the browser shows as a file is compared and replaced as a file, and `close()`, which replaces the file whole, is the web's atomic write.
- `folderHolds(folder, handle)` is true when `await folder.resolve(handle)` is an array holding exactly `handle.name`, and false when it is null (the file is not inside the folder), longer (the file is in a subfolder) or throws.

The desktop file service (Tauri dialogs) implements the same `FileService` type and is specified with the desktop host (open issue 2, desktop.md 3.7): it reaches the `assets/` folder next to every desktop handle through the host's `read_asset` and `write_asset` commands.

`src/files/recent.ts` and `src/files/recovery.ts`:

```ts
/** `folder` is the web folder that holds the file (3.19), null on the desktop and without one. */
export type RecentFile = { readonly id: string; readonly name: string; readonly openedAt: number; readonly handle: FileHandle | null; readonly folder: FileSystemDirectoryHandle | null; readonly text: string | null }
export type RecentFilesStore = {
  list(): Promise<readonly RecentFile[]>
  remember(file: { readonly name: string; readonly handle: FileHandle | null; readonly folder: FileSystemDirectoryHandle | null; readonly text: string }): Promise<void>
  forget(id: string): Promise<void>
}
export function createIndexedDbRecentFiles(factory: IDBFactory, now?: () => number): RecentFilesStore
export function createMemoryRecentFiles(now?: () => number): RecentFilesStore

/** One document session's recovery copy (3.19); `id` is the session's `DocumentState.recoveryId`. */
/** `folder` and `digest` are those of the session's file reference (2.8, 3.19). */
export type RecoveryEntry = { readonly id: string; readonly name: string; readonly text: string; readonly savedAt: number; readonly handle: FileHandle | null; readonly folder: FileSystemDirectoryHandle | null; readonly digest: string | null; readonly reason: 'autosave' | 'replaced' }
export type RecoveryStore = {
  /** The entries whose session no running page holds, newest `savedAt` first. */
  list(): Promise<readonly RecoveryEntry[]>
  /** Writes the entry under its `id`, replacing only an entry with the same `id`; resolves whether it was stored. */
  write(entry: RecoveryEntry): Promise<boolean>
  /** Removes the entry with this `id`; every other entry stays. */
  remove(id: string): Promise<void>
}
/** What the IndexedDB store uses besides the database: `navigator.locks` and `navigator.storage`, or null where the host lacks them. */
export type RecoveryEnvironment = { readonly locks: LockManager | null; readonly storage: StorageManager | null }
export function createIndexedDbRecovery(factory: IDBFactory, environment: RecoveryEnvironment): RecoveryStore
/** A store in memory, for tests and hosts without IndexedDB: no other page shares it, so `list` returns every entry. */
export function createMemoryRecovery(): RecoveryStore
```

- IndexedDB database `partsmith`, version 2, object stores `recent` (key path `id`) and `recovery` (key path `id`, one record per entry, so documents, tabs and windows never overwrite each other's copies). `main.tsx` passes `{ locks: navigator.locks ?? null, storage: navigator.storage ?? null }`.
- Upgrade from version 1, whose `recovery` store had key path `slot` and one record with slot `current`: inside the upgrade transaction the UI reads that record, deletes the old store, creates the new `recovery` store and, when the record is a valid entry without its `id` (the other fields of `RecoveryEntry`), puts it with a new `id` (`crypto.randomUUID()`) and without `slot`; an invalid record is dropped. `recent` and its records stay as they are, and a new database gets both stores. A page still running a version 1 build closes its connection on `versionchange` and then cannot open version 2 (`VersionError`), so its recent files and recovery copy fail as below until it reloads, and it never writes a version 1 record into the new store.
- Running sessions: when `write` stores an `autosave` entry, the store holds the Web Lock `partsmith-recovery-{id}` (`locks.request` in the default exclusive mode with `ifAvailable`, so a write never waits for a lock, and a callback that waits; the lock is held before the entry is stored) until it writes that `id` as `replaced` or removes it, or until the page goes away (the browser releases the locks of a closed or crashed page); writing the same `id` again keeps the one lock. `list` leaves out every entry whose lock any page holds (`locks.query()`), so a tab or window never offers, opens or discards a copy that another one is still writing. Without `locks`, or when a `locks` call fails, every entry is listed.
- The first `write` of a page asks the browser to keep the database under storage pressure: when `storage.persisted()` resolves false, it calls `storage.persist()` once (Firefox asks the user; Chromium and Safari decide on their own). The answer changes nothing, a rejection is ignored, and without `storage` nothing is called.
- `list` skips records that are not valid entries, without deleting them. A record without `folder` or `digest` (written before they existed) is valid, and reads them as null; so is a recent record without `folder`. A `folder` that is neither null nor a directory handle (`kind` `directory`), or a `digest` that is neither null nor a string, makes the record invalid.
- `remember` keeps at most 10 entries, most recent first. A new entry replaces an existing one for the same file: same handle (`isSameEntry` resolves true) or, for handle-less entries, the same name. Entries with a handle store `text: null`; entries without one store the text (so reopening works without file access). A new entry with `folder` null keeps the folder of the entry it replaces, so opening a file again with the picker keeps the folder an earlier session chose (3.19). `id` is `crypto.randomUUID()`.
- Every IndexedDB failure (private mode, quota, blocked) is caught: `list` resolves empty, `write` resolves false, `remember`, `forget`, `write` and `remove` change nothing, and a `console.warn` names the store. The app never depends on them working. A stored `write` resolves true, as every `write` of the memory store does.
- `upgradeDatabase(database, upgrade, oldVersion)` (`src/files/database.ts`) runs inside the open request's versionchange transaction: a new database (`oldVersion` 0) gets both stores at once, and the move of the version 1 copy runs only when `oldVersion` is 1, so an upgrade to a later version never redoes it.

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

The visual direction is the Partsmith direction in [docs/guidelines/README.md](../guidelines/README.md), applied in 3.23. Everything visual is a Tailwind utility class; the stylesheet adds only the two font faces and the theme variables that name them (2.16.1, 2.16.7).

#### 2.16.1 Stylesheet

`src/index.css` is exactly the maintainer-approved block of ADR 0013 with the font loading of ADR 0031 (667 bytes, LF line endings, one trailing newline, as oxfmt writes it):

```css
@import 'tailwindcss';

@font-face {
  font-family: 'Geist';
  font-style: normal;
  font-weight: 100 900;
  font-display: swap;
  src: url('/fonts/Geist-Variable.woff2') format('woff2');
}

@font-face {
  font-family: 'JetBrains Mono';
  font-style: normal;
  font-weight: 100 800;
  font-display: swap;
  size-adjust: 96%;
  src: url('/fonts/JetBrainsMono-Variable.woff2') format('woff2');
}

@theme {
  --font-sans: 'Geist', ui-sans-serif, system-ui, sans-serif;
  --font-sans--font-feature-settings: 'ss05';
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;
  --font-mono--font-feature-settings: normal;
}

@custom-variant dark (&:where(.dark, .dark *));
```

There is no other CSS file. The `@theme` block sets only Tailwind's two font theme variables and their feature settings (2.16.7); the app defines no other CSS variable, and Tailwind's palette and every other theme value stay as Tailwind 4.3.3 ships them. Tailwind's preflight reads `--font-sans` and `--font-sans--font-feature-settings` (as `--default-font-family` and `--default-font-feature-settings`) for the root element, and `--font-mono` and `--font-mono--font-feature-settings` for `font-mono` and for `code`, `kbd`, `samp` and `pre`. The `@font-face` rules load the files of `public/fonts` from the app's own origin (2.16.7). The `dark:` variant matches inside an element with the `dark` class. Changing this file needs human approval (ADRs 0013 and 0031, tailwind-css.md rule 2). U-91 checks it.

#### 2.16.2 The mapping module

`src/theme/roles.ts` is the single source of the role-to-color mapping and the UI scale classes. It is the documented design-system mapping that tailwind-css.md allows to be single-sourced; it holds color and scale classes only, never layout. The type roles are not in it: their faces are Tailwind theme variables (2.16.1), and components use the Tailwind utilities of 2.16.7. Every class string in it is a complete literal, so Tailwind's scanner finds it; nothing composes class names. Components combine the facets with their own inline layout classes through `clsx/lite`, for example `clsx('flex h-8 items-center gap-2 px-3', COLOR_CLASSES.text.foreground, COLOR_CLASSES.hover.background, selected && COLOR_CLASSES.selection.background)`.

```ts
export type ThemeName = 'light' | 'dark'
export type Facet =
  | 'background' | 'active' | 'foreground' | 'placeholder' | 'border' | 'outline' | 'inset' | 'stroke' | 'fill' | 'shadow'
  | 'banner' | 'opaque' | 'overflowTop' | 'overflowBottom' | 'overlay'

/** sRGB '#rrggbb' of every Tailwind palette step the mapping uses (2.16.5), declared `as const`. */
export const PALETTE_HEX: { readonly white: '#ffffff'; readonly 'zinc-50': '#fafafa' /* ...one entry per row of 2.16.5 */ }
/** A palette step the mapping uses, such as 'zinc-900' or 'white'. */
export type PaletteStep = keyof typeof PALETTE_HEX

/** Interface roles and their facets as literal class strings (2.16.3), declared `as const`. */
export const COLOR_CLASSES: { readonly surface: { readonly background: 'bg-white dark:bg-zinc-900' } /* ...one entry per role of 2.16.3 */ }
export type ColorRole = keyof typeof COLOR_CLASSES
/** The role of each world axis, in the interface (2.16.3) and in the viewport (2.16.4). */
export const AXIS_ROLES: { readonly X: 'axisX'; readonly Y: 'axisY'; readonly Z: 'axisZ' }

export type ViewportRole =
  | 'viewport' | 'body' | 'edgeDark' | 'edgeLight' | 'highlight' | 'problem'
  | 'sketchUnder' | 'sketchFull' | 'sketchOver' | 'sketchConstruction' | 'sketchRegion' | 'sketchReference'
  | 'datum' | 'bed' | 'grid' | 'analysisLow' | 'analysisHigh' | 'axisX' | 'axisY' | 'axisZ'
/** Palette step of each viewport role per theme (2.16.4). */
export const VIEWPORT_STEPS: Readonly<Record<ViewportRole, Readonly<Record<ThemeName, PaletteStep>>>>
/** '#rrggbb' of each viewport role per theme: PALETTE_HEX[VIEWPORT_STEPS[role][theme]]. */
export const VIEWPORT_COLORS: Readonly<Record<ThemeName, Readonly<Record<ViewportRole, string>>>>

/** Body color swatches of the color popover (3.10), in order; `hex` null is Default (removes `Body.color`). */
export const BODY_SWATCHES: readonly { readonly name: string; readonly step: PaletteStep; readonly background: string; readonly hex: string | null }[]

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
| `stage` | background | `bg-zinc-100 dark:bg-zinc-950` | The app root behind the islands: the viewport's step (`viewport`, 2.16.4), seen in the gutter before the first frame, while the WebGL context is lost and when the viewport region has crashed (3.2) |
| `surface` | background | `bg-white dark:bg-zinc-900` | Every island, card and chip (3.2), a step lighter than `viewport` in both themes: the frame islands (top bar, browser, side panel, timeline, status bar), the inspector, the sketch dimension editor, the viewport controls, the view cube's faces and its Home view button, the sketch header, the legend, the core panels and the cards, chips and marks over the viewport (empty state, hints, plane and axis labels, constraint and inference glyphs, dimension labels); the unsupported page (3.1) |
| `surfaceRaised` | background | `bg-white dark:bg-zinc-800` | What floats over the islands and cards (3.2): menus, popovers, tooltips, the `expression-suggestions` list, the palette, notifications and the drop overlay; a step lighter than `surface` in the dark theme, and white like it in the light theme, where the stronger shadow sets them apart |
| | overlay | `@max-5xl/app:bg-white @max-5xl/app:dark:bg-zinc-800` | The browser and the side panel while they float over the free area as overlay islands (under 65rem, 3.2), beside their `surface` background: they come and go over the free area's cards, so in the dark theme they take the step above them |
| `field` | background | `bg-white dark:bg-zinc-950` | Text inputs, selects, selection boxes |
| `hover` | background | `hover:bg-zinc-200 dark:hover:bg-zinc-700/60` | Hovered rows, menu items, toolbar, icon and ghost buttons (Search, Export), and default buttons over their `tonal` fill (3.23); translucent in the dark theme, so one class reads on `surface` and `surfaceRaised` (2.16.6), and always laid over an opaque surface |
| | active | `bg-zinc-200 dark:bg-zinc-700/60` | The palette's active option, a menu's keyboard-active item, the cells of the hovered target on a view cube face (3.14), and the pressed state of the top bar's Toggle browser and Dark theme (3.23) |
| | foreground | `hover:text-zinc-900 dark:hover:text-zinc-100` | Unchecked segments of segmented controls, whose `tonal` track leaves no room for a hover fill: the label turns `text` on hover (3.23) |
| | banner | `hover:bg-zinc-950/10 dark:hover:bg-white/15` | Hovered default buttons on a status banner (`variant="banner"`, 3.23) |
| | opaque | `hover:bg-zinc-200 dark:hover:bg-zinc-800` | The hover of a chip that is itself the surface over the viewport (the sketch header, the view cube's Home view button), where a translucent fill would let the canvas show through it |
| `tonal` | background | `bg-zinc-100 dark:bg-zinc-700/40` | Default buttons (every `Button` that is not primary), segmented control tracks, the read-only JSON block of an unsupported field (2.12); translucent in the dark theme, like `hover` |
| | banner | `bg-zinc-950/5 dark:bg-white/10` | Default buttons on a status banner (`inspector-error`, `variant="banner"`, 3.23), whose `errorSubtle` and `warningSubtle` fills the `background` facet nearly matches (2.16.6) |
| `thumb` | background | `bg-white dark:bg-zinc-700` | The checked segment of a segmented control, raised over its `tonal` track (3.23) |
| `selection` | background | `bg-orange-100 dark:bg-orange-950` | Selected rows and timeline items, the print check's focused finding, pressed toggle buttons (a toggle `Button` among them, 3.23) other than the top bar's Toggle browser and Dark theme, the active tool (none of these change on hover) |
| `forcedEdge` | border | `border-transparent` | The 1 CSS pixel `border` (Tailwind's `border`) of every island, card, popup and chip, of the default `Button` and of a disabled primary button (an enabled one's border is its fill, `accent`), segmented control tracks, the view cube's Home view button, constraint and inference glyphs and dimension labels (`accent` while selected), the read-only JSON block and a `Switch`'s thumb, and the rule of a scrolling body at rest (3.2): nothing draws it, so depth is the tonal step and the shadow (3.2) and every box keeps the size its old outline gave it, and in forced-colors mode the system paints it as the element's edge (3.24) |
| | outline | `outline-transparent` | The resting outline color of every element that can show a state outline (3.23): the segments of segmented controls and every element shown selected, pressed or active by a fill; in that state the element adds `outline -outline-offset-1`, a 1 CSS pixel inner outline drawn only in forced-colors mode (3.24) |
| `divider` | border | `border-zinc-200 dark:border-white/10` | The few hairlines left inside islands and popups, where groups cannot be told apart otherwise (3.2): menu separators and the rule under the palette's search field; white at 10 percent in the dark theme, so one class reads on `surface` and `surfaceRaised`; decorative |
| | overflowTop | `data-overflow-top:border-t-zinc-200 dark:data-overflow-top:border-t-white/10` | The top rule of a scrolling body under a fixed header, drawn while content is scrolled under the header (3.2) |
| | overflowBottom | `data-overflow-bottom:border-b-zinc-200 dark:data-overflow-bottom:border-b-white/10` | The bottom rule of a scrolling body above a fixed footer, drawn while content continues under the footer (3.2) |
| `elevation` | shadow | `shadow-black/10 dark:shadow-black/20` | The color of every `shadow-*` class (3.2, 3.23): black at 10 percent in the light theme (Tailwind's own for `shadow-md` and `shadow-lg`, twice its 5 percent for `shadow-xs`) and at 20 percent in the dark theme. The shadows fall below and beside an element, never above it: in the light theme they mark an island's lower and side edges on the viewport; on the near-black dark viewport no shadow shows at any strength, and over the model the dark shadow darkens a body by at most about 1.6:1 just under a lower edge, soft depth rather than a dark halo (2.16.6) |
| `borderControl` | border | `border-zinc-500` | Boundaries of inputs, selects, switches, swatches and selection boxes |
| `text` | foreground | `text-zinc-900 dark:text-zinc-100` | Primary text and icons |
| `textMuted` | foreground | `text-zinc-600 dark:text-zinc-400` | Secondary text, captions, units, palette details, idle icons, the info notification icon |
| | placeholder | `placeholder:text-zinc-600 dark:placeholder:text-zinc-400` | Input placeholders, such as the live measured value in an empty draw input (3.16.4) |
| `textDisabled` | foreground | `text-zinc-400 dark:text-zinc-600` | Disabled text and icons (incidental, exempt from contrast) |
| `accent` | background | `bg-orange-600 dark:bg-orange-500` | Primary button fill (OK, Finish sketch, Export 3MF, Export), the rollback marker line |
| | foreground | `text-zinc-950` | Text and icons on the accent fill |
| | border | `border-orange-600 dark:border-orange-500` | The active selection field; required empty selection fields (with `border-dashed`); primary buttons, the toolbar's included, in the color of their fill, so forced colors draw their edge (3.23, 3.24); a selected constraint glyph or dimension label (3.16.2) |
| | outline | `outline-orange-600 dark:outline-orange-500` | The timeline item of the feature being edited and of the sketch being edited (with `outline-2`) |
| | stroke | `stroke-orange-600 dark:stroke-orange-500` | The box selection rectangle (SVG overlay) |
| `accentHover` | background | `hover:bg-orange-500 dark:hover:bg-orange-400` | Primary button hover |
| `focusRing` | outline | `focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-orange-600 dark:focus-visible:outline-orange-500` | Keyboard focus on every interactive element |
| | inset | `focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-orange-600 dark:focus-visible:outline-orange-500` | Keyboard focus on the controls of islands under 2.5rem tall (the status bar's items and its health badge, the collapsed timeline's buttons), drawn inside the control so the island's edge cannot clip it (3.23) |
| `errorMark` | foreground | `text-red-700 dark:text-red-500` | Error icons (`circle-x`) |
| | border | `border-red-700 dark:border-red-500` | Invalid field borders |
| | stroke | `stroke-red-700 dark:stroke-red-500` | Profile issue rings and finding markers (SVG overlay) |
| `errorText` | foreground | `text-red-700 dark:text-red-300` | Error messages, error states of the sketch header, parameter values and status bar |
| `errorSubtle` | background | `bg-red-50 dark:bg-red-950` | Error banners (`inspector-error` with an error) |
| `errorFill` | background | `bg-red-600` | Chips of conflicting constraint glyphs and dimension labels |
| | foreground | `text-white` | Text and icons on the error fill |
| `warningMark` | fill | `fill-yellow-400` | Fill of the `triangle-alert` icon, drawn as a hazard sign |
| | foreground | `text-zinc-950` | Outline and exclamation mark of that icon (lucide strokes use `currentColor`) |
| | border | `border-yellow-800 dark:border-yellow-300` | A field whose value is above the largest that works (3.7) |
| | background | `bg-yellow-800 dark:bg-yellow-300` | The timeline's insertion bar while a drop would break a use (3.9) |
| `warningText` | foreground | `text-yellow-800 dark:text-yellow-300` | Warning text (the outline and extra-constraint suffixes of the sketch header, 3.16.8) |
| `warningSubtle` | background | `bg-yellow-50 dark:bg-yellow-950` | Warning banners (`inspector-error` with warnings only) |
| `warningFill` | background | `bg-yellow-400` | Chips of redundant constraint glyphs and dimension labels |
| | foreground | `text-zinc-950` | Text and icons on the warning fill |
| `successMark` | foreground | `text-green-700 dark:text-green-400` | Success icons (`circle-check`) |
| `analysis` | background | `bg-linear-to-r/srgb from-sky-200 to-violet-700` | The overhang legend gradient, interpolated in sRGB like the viewport ramp (3.12 pass 3) |
| `axisX` | foreground | `text-rose-600 dark:text-rose-400` | The letter X: coordinate fields (3.7), coordinate readouts (3.8), the viewport's X axis label (3.12) and a sketch's X axis label (3.16.2); the icon of the browser's X axis row (3.10) |
| | border | `border-rose-600 dark:border-rose-400` | The view cube's edges along X (3.14) |
| `axisY` | foreground | `text-emerald-700 dark:text-lime-400` | The letter Y in the same places; the icon of the Y axis row |
| | border | `border-emerald-700 dark:border-lime-400` | The view cube's edges along Y |
| `axisZ` | foreground | `text-sky-700 dark:text-sky-300` | The letter Z in the same places; the icon of the Z axis row |
| | border | `border-sky-700 dark:border-sky-300` | The view cube's edges along Z |

Color carries meaning and nothing else: orange (`accent`, `selection`, `highlight`) means selection, focus, the current tool and the primary action; blue means under-constrained sketch geometry and closed regions; red means errors and conflicts; a yellow hazard sign means a warning; green means success; sky to violet is the analysis ramp. Warnings never use orange, and errors never use the accent's steps. The axes are the one second meaning, kept apart by hues and steps of their own: rose (`axisX`), emerald in the light theme and lime in the dark theme (`axisY`) and sky (`axisZ`) mean the world axes X, Y and Z, never a status. An axis color always comes with its letter, except on the origin planes (tinted by their normal, named by their plane label, 3.12) and the view cube's edges (named by its face labels, 3.14). The values need human review at the gate (open issue 13).

#### 2.16.4 Viewport color roles

The WebGL viewport needs numbers. `VIEWPORT_STEPS` holds the steps below and `VIEWPORT_COLORS` their hex values from `PALETTE_HEX`, so DOM and viewport draw from the same palette steps. Roles shared with the interface use its steps: `viewport` is `stage`, `highlight` is `accent`, `problem` and `sketchOver` are `errorMark`, `body` is the Default swatch, and `axisX`, `axisY` and `axisZ` are the interface roles of the same names.

| Role | Light | Dark | Use |
| --- | --- | --- | --- |
| `viewport` | `zinc-100` `#f4f4f5` | `zinc-950` `#09090b` | Background, one flat tone |
| `body` | `zinc-400` `#9f9fa9` | `zinc-400` `#9f9fa9` | Default body color (no `Body.color`) |
| `edgeDark` | `zinc-900` `#18181b` | `zinc-900` `#18181b` | Edges on light bodies |
| `edgeLight` | `zinc-200` `#e4e4e7` | `zinc-200` `#e4e4e7` | Edges on dark bodies |
| `highlight` | `orange-600` `#f54900` | `orange-500` `#ff6900` | Hover and selection tints, hovered and selected sketch items, snap targets |
| `problem` | `red-700` `#c10007` | `red-500` `#fb2c36` | Problem tints, profile issue halos, and the split planes of the focused print finding, filled at 15 percent and outlined at 60 percent (3.17) |
| `sketchUnder` | `blue-600` `#155dfc` | `blue-400` `#51a2ff` | Under-constrained sketch geometry |
| `sketchFull` | `zinc-900` `#18181b` | `zinc-50` `#fafafa` | Fully constrained sketch geometry |
| `sketchOver` | `red-700` `#c10007` | `red-500` `#fb2c36` | Over-constrained (conflicting) sketch geometry |
| `sketchConstruction` | `zinc-600` `#52525c` | `zinc-300` `#d4d4d8` | Fully constrained construction geometry, dashed (3.16.2) |
| `sketchRegion` | `blue-500` `#2b7fff` | `blue-400` `#51a2ff` | Closed region fill, at 12 percent opacity |
| `sketchReference` | `zinc-500` `#71717b` | `zinc-400` `#9f9fa9` | Other sketches, dimension lines, the origin point, the axes of a sketch on a datum plane or a face (3.16.2) |
| `datum` | `zinc-400` `#9f9fa9` | `zinc-500` `#71717b` | Datum plane fill at 15 percent, outline at 60 percent |
| `bed` | `zinc-500` `#71717b` | `zinc-400` `#9f9fa9` | Printer bed outline |
| `grid` | `zinc-300` `#d4d4d8` | `zinc-700` `#3f3f46` | Ground and sketch grid lines (minor at 35 percent, major at 70 percent opacity, 3.12) |
| `analysisLow` | `sky-200` `#b8e6fe` | `sky-200` `#b8e6fe` | Overhang color map at the limit angle |
| `analysisHigh` | `violet-700` `#7008e7` | `violet-700` `#7008e7` | Overhang color map at 90 degrees |
| `axisX` | `rose-600` `#ec003f` | `rose-400` `#ff637e` | The origin X axis, the Right plane (YZ, normal X) at 15 percent fill and 60 percent outline, a sketch axis along X on an origin plane (3.12, 3.16.2) |
| `axisY` | `emerald-700` `#007a55` | `lime-400` `#9ae600` | The origin Y axis, the Front plane (XZ, normal Y), a sketch axis along Y on an origin plane |
| `axisZ` | `sky-700` `#0069a8` | `sky-300` `#74d4ff` | The origin Z axis, the Top plane (XY, normal Z), a sketch axis along Z on an origin plane |

The grid is decorative and never the only signal of anything (the status bar states its spacing, 3.21), so its rows in 2.16.6 are measured, with no threshold.

The viewport is one flat tone, a step darker than the islands in both themes (`zinc-100` under white islands, `zinc-950` under `zinc-900` ones, 3.2): an elevated surface is lighter than what it sits on, and the chrome recedes through quiet color, size and weight, not through darkness. A gradient would bring some point of the viewport closer to the islands' tone, and the light viewport cannot go darker: on `zinc-200` the accent's marks (selected edges, hovered sketch items, snap targets) would fall to 2.83:1, under 3:1. The model stays the hero on it; 2.16.6 measures its colors against it. Edges are drawn in `edgeDark` or `edgeLight`, whichever has the higher contrast ratio against the body's base color, so they stay visible on any body color. The analysis ramp is cool because orange, yellow and red already mean selection, warnings and errors; both of its ends lie inside the sRGB gamut, so the legend and the viewport show the same colors.

`viewportTheme(theme)` returns `linearFromHex` of `viewport` (as `background`), `highlight`, `problem`, `edgeDark` and `edgeLight`; the scene converts the body, sketch, datum, axis and bed colors with `linearFromHex` for each draw, and passes `encodedFromHex` of the analysis ends in `RenderFrame.analysis`.

#### 2.16.5 Palette steps and body swatches

Tailwind 4.3.3 defines its palette in OKLCH. The hex of a step is the CSS Color 4 conversion of its OKLCH value (OKLCH to OKLab to linear sRGB), each channel clipped to [0, 1], sRGB-encoded and rounded to 8 bits. Steps marked clipped lie partly outside the sRGB gamut; on wide-gamut displays the DOM shows them a little more saturated than the hex the viewport draws (open issue 11). U-90 recomputes every value from `tailwindcss/colors`.

| Step | Tailwind 4.3.3 value | sRGB hex | In sRGB gamut |
| --- | --- | --- | --- |
| `white` | `#fff` | `#ffffff` | yes |
| `black` | `#000` | `#000000` | yes |
| `zinc-50` | `oklch(98.5% 0 none)` | `#fafafa` | yes |
| `zinc-100` | `oklch(96.7% 0.001 286.375)` | `#f4f4f5` | yes |
| `zinc-200` | `oklch(92% 0.004 286.32)` | `#e4e4e7` | yes |
| `zinc-300` | `oklch(87.1% 0.006 286.286)` | `#d4d4d8` | yes |
| `zinc-400` | `oklch(70.5% 0.015 286.067)` | `#9f9fa9` | yes |
| `zinc-500` | `oklch(55.2% 0.016 285.938)` | `#71717b` | yes |
| `zinc-600` | `oklch(44.2% 0.017 285.786)` | `#52525c` | yes |
| `zinc-700` | `oklch(37% 0.013 285.805)` | `#3f3f46` | yes |
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
| `rose-400` | `oklch(71.2% 0.194 13.428)` | `#ff637e` | no, clipped |
| `rose-600` | `oklch(58.6% 0.253 17.585)` | `#ec003f` | no, clipped |
| `lime-400` | `oklch(84.1% 0.238 128.85)` | `#9ae600` | no, clipped |
| `green-400` | `oklch(79.2% 0.209 151.711)` | `#05df72` | yes |
| `green-600` | `oklch(62.7% 0.194 149.214)` | `#00a63e` | no, clipped |
| `green-700` | `oklch(52.7% 0.154 150.069)` | `#008236` | no, clipped |
| `emerald-700` | `oklch(50.8% 0.118 165.612)` | `#007a55` | no, clipped |
| `teal-600` | `oklch(60% 0.118 184.704)` | `#009689` | no, clipped |
| `sky-200` | `oklch(90.1% 0.058 230.902)` | `#b8e6fe` | yes |
| `sky-300` | `oklch(82.8% 0.111 230.318)` | `#74d4ff` | no, clipped |
| `sky-700` | `oklch(50% 0.134 242.749)` | `#0069a8` | no, clipped |
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

WCAG 2.2 AA: text at least 4.5:1 (every text pair is held to 4.5:1, large or not) and non-text at least 3:1 (control boundaries, focus indicators, meaningful icons and viewport geometry). The ratios are computed from the hex values of 2.16.5 with the WCAG relative luminance, truncated to two decimals; U-70 recomputes each pair and checks that it equals the table and meets the threshold. A role with an opacity (in the dark theme `hover`, `tonal` and `divider`, and in both themes the `banner` facets) is measured composited over the role named after "over", per sRGB-encoded channel rounded to 8 bits (halves up), as the browser blends it; the light `hover` and `tonal` are opaque, so their rows over `surface` and over `surfaceRaised` (both white) agree.

Text:

| Text role | On | Required | Light | Dark |
| --- | --- | --- | --- | --- |
| `text` | `surface` | 4.5 | 17.71 | 16.11 |
| `text` | `surfaceRaised` | 4.5 | 17.71 | 13.55 |
| `text` | `field` | 4.5 | 17.71 | 18.10 |
| `text` | `hover` over `surface` | 4.5 | 13.96 | 12.10 |
| `text` | `hover` over `surfaceRaised` | 4.5 | 13.96 | 11.08 |
| `text` | `hover` opaque | 4.5 | 13.96 | 13.55 |
| `text` | `tonal` over `surface` | 4.5 | 16.11 | 13.35 |
| `text` | `tonal` over `surfaceRaised` | 4.5 | 16.11 | 11.78 |
| `text` | `thumb` | 4.5 | 17.71 | 9.50 |
| `text` | `selection` | 4.5 | 15.45 | 14.24 |
| `text` | `errorSubtle` | 4.5 | 16.19 | 14.70 |
| `text` | `warningSubtle` | 4.5 | 17.12 | 13.20 |
| `text` | `tonal` banner over `errorSubtle` | 4.5 | 14.54 | 11.47 |
| `text` | `tonal` banner over `warningSubtle` | 4.5 | 15.42 | 9.84 |
| `text` | `hover` banner over `errorSubtle` | 4.5 | 13.09 | 9.87 |
| `text` | `hover` banner over `warningSubtle` | 4.5 | 13.82 | 8.39 |
| `textMuted` | `surface` | 4.5 | 7.71 | 6.75 |
| `textMuted` | `surfaceRaised` | 4.5 | 7.71 | 5.67 |
| `textMuted` | `field` | 4.5 | 7.71 | 7.58 |
| `textMuted` | `hover` over `surface` | 4.5 | 6.08 | 5.07 |
| `textMuted` | `hover` over `surfaceRaised` | 4.5 | 6.08 | 4.64 |
| `textMuted` | `hover` opaque | 4.5 | 6.08 | 5.67 |
| `textMuted` | `tonal` over `surface` | 4.5 | 7.02 | 5.59 |
| `textMuted` | `tonal` over `surfaceRaised` | 4.5 | 7.02 | 4.93 |
| `textMuted` | `selection` | 4.5 | 6.73 | 5.96 |
| `errorText` | `surface` | 4.5 | 6.42 | 9.23 |
| `errorText` | `surfaceRaised` | 4.5 | 6.42 | 7.76 |
| `errorText` | `errorSubtle` | 4.5 | 5.86 | 8.42 |
| `warningText` | `surface` | 4.5 | 6.84 | 13.35 |
| `warningText` | `surfaceRaised` | 4.5 | 6.84 | 11.22 |
| `warningText` | `warningSubtle` | 4.5 | 6.61 | 10.93 |
| `onAccent` | `accent` | 4.5 | 5.52 | 6.89 |
| `onAccent` | `accentHover` | 4.5 | 6.89 | 8.36 |
| `onWarningFill` | `warningFill` | 4.5 | 12.64 | 12.64 |
| `onErrorFill` | `errorFill` | 4.5 | 4.76 | 4.76 |
| `axisX` | `field` | 4.5 | 4.52 | 6.95 |
| `axisX` | `surface` | 4.5 | 4.52 | 6.19 |
| `axisY` | `field` | 4.5 | 5.36 | 12.97 |
| `axisY` | `surface` | 4.5 | 5.36 | 11.55 |
| `axisZ` | `field` | 4.5 | 5.85 | 11.94 |
| `axisZ` | `surface` | 4.5 | 5.85 | 10.63 |

Non-text (`warningMark` is measured on its `zinc-950` outline in the light theme and on its `yellow-400` fill in the dark theme, the part that contrasts with the surface, and its border and background facets on their own steps, `yellow-800` light and `yellow-300` dark; `accent` on `selection` is the focus ring of a selected row, on `thumb` and `tonal` that of a segment, and on `errorSubtle` and `warningSubtle` that of a banner's buttons):

| Mark role | Against | Required | Light | Dark |
| --- | --- | --- | --- | --- |
| `borderControl` | `surface` | 3 | 4.82 | 3.67 |
| `borderControl` | `surfaceRaised` | 3 | 4.82 | 3.08 |
| `borderControl` | `field` | 3 | 4.82 | 4.12 |
| `accent` | `surface` | 3 | 3.59 | 6.13 |
| `accent` | `surfaceRaised` | 3 | 3.59 | 5.15 |
| `accent` | `selection` | 3 | 3.13 | 5.42 |
| `accent` | `tonal` over `surface` | 3 | 3.27 | 5.08 |
| `accent` | `tonal` over `surfaceRaised` | 3 | 3.27 | 4.48 |
| `accent` | `thumb` | 3 | 3.59 | 3.61 |
| `accent` | `errorSubtle` | 3 | 3.28 | 5.59 |
| `accent` | `warningSubtle` | 3 | 3.47 | 5.02 |
| `accent` | `viewport` | 3 | 3.27 | 6.89 |
| `errorMark` | `surface` | 3 | 6.42 | 4.65 |
| `errorMark` | `surfaceRaised` | 3 | 6.42 | 3.91 |
| `errorMark` | `viewport` | 3 | 5.84 | 5.22 |
| `errorMark` | `hover` over `surface` | 3 | 5.06 | 3.49 |
| `errorMark` | `hover` over `surfaceRaised` | 3 | 5.06 | 3.19 |
| `successMark` | `surface` | 3 | 4.94 | 9.95 |
| `successMark` | `surfaceRaised` | 3 | 4.94 | 8.37 |
| `warningMark` | `surface` | 3 | 19.89 | 11.25 |
| `warningMark` | `surfaceRaised` | 3 | 19.89 | 9.46 |
| `warningMark` | `hover` over `surface` | 3 | 15.67 | 8.45 |
| `warningMark` | `hover` over `surfaceRaised` | 3 | 15.67 | 7.74 |
| `warningMark` | `selection` | 3 | 17.35 | 9.94 |
| `warningMark` border | `field` | 3 | 6.84 | 14.99 |
| `warningMark` background | `surface` | 3 | 6.84 | 13.35 |
| `sketchUnder` | `viewport` | 3 | 4.77 | 7.54 |
| `sketchFull` | `viewport` | 3 | 16.11 | 19.06 |
| `sketchOver` | `viewport` | 3 | 5.84 | 5.22 |
| `sketchConstruction` | `viewport` | 3 | 7.02 | 13.46 |
| `sketchReference` | `viewport` | 3 | 4.39 | 7.58 |
| `bed` | `viewport` | 3 | 4.39 | 7.58 |
| `axisX` | `viewport` | 3 | 4.11 | 6.95 |
| `axisX` | `surface` | 3 | 4.52 | 6.19 |
| `axisX` | `hover` over `surface` | 3 | 3.56 | 4.64 |
| `axisX` | `surfaceRaised` | 3 | 4.52 | 5.20 |
| `axisX` | `hover` over `surfaceRaised` | 3 | 3.56 | 4.25 |
| `axisX` | `selection` | 3 | 3.94 | 5.46 |
| `axisY` | `viewport` | 3 | 4.88 | 12.97 |
| `axisY` | `surface` | 3 | 5.36 | 11.55 |
| `axisY` | `hover` over `surface` | 3 | 4.22 | 8.67 |
| `axisY` | `surfaceRaised` | 3 | 5.36 | 9.71 |
| `axisY` | `hover` over `surfaceRaised` | 3 | 4.22 | 7.94 |
| `axisY` | `selection` | 3 | 4.67 | 10.20 |
| `axisZ` | `viewport` | 3 | 5.32 | 11.94 |
| `axisZ` | `surface` | 3 | 5.85 | 10.63 |
| `axisZ` | `hover` over `surface` | 3 | 4.61 | 7.98 |
| `axisZ` | `surfaceRaised` | 3 | 5.85 | 8.93 |
| `axisZ` | `hover` over `surfaceRaised` | 3 | 4.61 | 7.31 |
| `axisZ` | `selection` | 3 | 5.10 | 9.39 |

The axis rows cover the letters on fields, readouts and viewport chips (text), the origin axes, planes and sketch axes on the viewport, the browser's axis icons on rows at rest, hovered and selected (on `surfaceRaised` too, while the browser floats as an overlay island, 3.2), and the view cube's edges on its `surface` faces (the text rows' ratios). The light theme's `axisX` letters sit closest to the limit (4.52), which is why X is `rose-600` and not the brighter `rose-500` (3.75 on white).

The lowest ratios are the dark `borderControl` on `surfaceRaised` (3.08), the light theme's `accent` marks (3.13 to 3.59), the dark `errorMark` on a hovered row over `surfaceRaised` (3.19, the Problems list while the side panel floats as an overlay island) and the dark `accent` on `thumb` (3.61); the light marks use `orange-600` because the brighter `orange-500` falls below 3:1 on white (2.89). Distinctness, as OKLab distances from the OKLCH values (about 0.02 is one just-noticeable difference): `accent` and `errorMark` 0.150 light and 0.113 dark (hues 14 and 22 degrees apart, and the light red is darker); `accent` and the warning yellow 0.275 and 0.214 (51 and 44 degrees apart); `accent` and the warning insertion bar (`warningMark` background) 0.209 and 0.263, the light bar also much darker; `errorText` and `warningText` 0.138 and 0.217; `accent` and `sketchUnder` 0.448 and 0.368. The axis roles, light and dark: `axisX` and `errorMark` 0.099 and 0.097, `axisX` and `errorText` 0.099 and 0.126 (rose sits 10 and 12 degrees of hue from the error reds, and the light rose is lighter than `red-700`), `axisX` and `accent` 0.118 and 0.121; `axisY` and `successMark` 0.055 and 0.105 (emerald and lime, not green, 16 and 23 degrees apart); `axisZ` and `sketchUnder` 0.136 and 0.144, and `sketchRegion` 0.155 and 0.144 (sky, not blue, 20 and 24 degrees apart, and the dark sky much lighter: `sky-300`, because `sky-400` lay only 0.073 from `blue-400`, and under-constrained construction lines are dashed like the axes, 3.16.2); the dark `axisZ` and `analysisLow` 0.090 (the overhang map colors faces in Print context only); between the axes `axisX` and `axisY` 0.367 and 0.388, `axisY` and `axisZ` 0.158 and 0.282, `axisX` and `axisZ` 0.370 and 0.313. Under red-green color blindness (the full-severity matrices of Machado, Oliveira and Fernandes 2009 applied in linear sRGB): X and Y keep 0.129 and 0.168 apart for deuteranopia and 0.079 and 0.305 for protanopia, so the light theme's X and Y come close to each other there; the light `axisY` keeps 0.192 and 0.068 from `accent` and 0.087 and 0.156 from `errorMark` (it is `emerald-700` because `lime-700` lay 0.014 from `accent` under protanopia and 0.010 from `errorMark` under deuteranopia); the dark `axisY` comes within 0.019 (deuteranopia) of the warning yellow `yellow-400` and 0.016 (protanopia) of `yellow-300`, and warnings always pair those with the hazard shape or with words; the dark `axisZ` keeps 0.150 and 0.145 from `blue-400`. The letter beside every axis color carries the meaning (3.24), and hovered and selected axes are also drawn thicker (3.12). Hue is never the only signal (3.24).

Tonal steps and the model on the viewport, measured for the design review (open issue 13) and recomputed by U-70, with no threshold: the tonal steps are decorative (a checked segment is named by its semibold `text` label and a button by its label, and forced colors draw every edge and state, 3.23, 3.24), and the model's outline is carried by its edges, labels and thicker hover strokes. A percentage composites the color over what it lies on, per sRGB-encoded channel rounded to 8 bits, as the default framebuffer blends; a hovered or selected body mixes `highlight` into `body` the same way (3.12 pass 2); "over" composites as in the tables above.

| Color | Against | Light | Dark |
| --- | --- | --- | --- |
| `surface` | `viewport` | 1.09 | 1.12 |
| `surfaceRaised` | `surface` | 1.00 | 1.18 |
| `surfaceRaised` | `viewport` | 1.09 | 1.33 |
| `hover` over `surface` | `surface` | 1.26 | 1.33 |
| `hover` over `surfaceRaised` | `surfaceRaised` | 1.26 | 1.22 |
| `tonal` over `surface` | `surface` | 1.09 | 1.20 |
| `tonal` over `surfaceRaised` | `surfaceRaised` | 1.09 | 1.14 |
| `hover` over `surface` | `tonal` over `surface` | 1.15 | 1.10 |
| `hover` over `surfaceRaised` | `tonal` over `surfaceRaised` | 1.15 | 1.06 |
| `selection` | `tonal` over `surface` | 1.04 | 1.06 |
| `thumb` | `tonal` over `surface` | 1.09 | 1.40 |
| `thumb` | `tonal` over `surfaceRaised` | 1.09 | 1.24 |
| `textMuted` | `thumb` | 7.71 | 3.98 |
| `field` | `surface` | 1.00 | 1.12 |
| `field` | `surfaceRaised` | 1.00 | 1.33 |
| `divider` over `surface` | `surface` | 1.26 | 1.32 |
| `divider` over `surfaceRaised` | `surfaceRaised` | 1.26 | 1.37 |
| `tonal` banner over `errorSubtle` | `errorSubtle` | 1.11 | 1.28 |
| `tonal` banner over `warningSubtle` | `warningSubtle` | 1.11 | 1.34 |
| `hover` banner over `errorSubtle` | `errorSubtle` | 1.23 | 1.48 |
| `hover` banner over `warningSubtle` | `warningSubtle` | 1.23 | 1.57 |
| `body` | `viewport` | 2.38 | 7.58 |
| `body` hovered (`highlight` at 20 percent) | `viewport` | 2.70 | 7.08 |
| `body` selected (`highlight` at 35 percent) | `viewport` | 2.92 | 6.86 |
| `edgeDark` | `body` | 6.75 | 6.75 |
| `edgeDark` | `viewport` | 16.11 | 1.12 |
| `edgeLight` | `viewport` | 1.15 | 15.67 |
| `grid` minor (35 percent) | `viewport` | 1.10 | 1.17 |
| `grid` major (70 percent) | `viewport` | 1.22 | 1.49 |
| `datum` outline (60 percent) | `viewport` | 1.63 | 2.15 |
| `axisX` plane outline (60 percent) | `viewport` | 2.82 | 3.08 |
| `axisY` plane outline (60 percent) | `viewport` | 2.45 | 5.08 |
| `axisZ` plane outline (60 percent) | `viewport` | 2.56 | 4.75 |
| `sketchRegion` (12 percent) | `viewport` | 1.14 | 1.14 |
| `analysisLow` | `viewport` | 1.21 | 14.95 |
| `analysisHigh` | `viewport` | 6.63 | 2.72 |

The light tonal steps are small in WCAG terms (in OKLCH lightness white is 100 and `zinc-100` 96.7 percent, 3.3 apart, while `zinc-950`, `zinc-900`, `zinc-800` and `zinc-700` lie at 14.1, 21, 27.4 and 37 percent, 6.4 to 9.6 apart). `textMuted` on `thumb` is a disabled control's checked label (3.23), exempt as disabled text; `selection` against `tonal` is a pressed toggle `Button` against an unpressed one, which reads by hue (open issue 13). The shadows add to the steps only below and beside an element, since Tailwind's `shadow-md` and `shadow-lg` are offset downward and start inside the top edge. Estimated with a Gaussian model of their two layers at Tailwind's offsets, blurs and spreads, half a pixel outside the edge: a light island on the viewport reads about 1.10 above, 1.20 beside and 1.42 below it, against the step's 1.09; a white popup over a white island 1.00 above, 1.09 beside and 1.32 below; a dark island on the viewport 1.12 to 1.13 on every side, the step's own 1.12, so no dark shadow shows there at any strength; a dark popup over an island, and a dark overlay island over a card (both `surfaceRaised` over `surface`), 1.18 above, 1.21 beside and 1.25 below, and a popup over another popup or over an overlay island 1.00, 1.03 and 1.11; a thumb over its track 1.11, 1.18 and 1.28 in the light theme and 1.42 to 1.51 in the dark theme. Over the default body the light shadow darkens the model by at most 1.27 and the dark one by about 1.6 just under a lower edge (black at 40 percent would reach 2.6, a dark halo). Shadow offsets and blurs are CSS pixels, not rem (`shadow-md` reaches about 8 pixels below an edge and `shadow-lg` 20), so at 200 and 300 percent UI scale a shadow reaches half and a third as far relative to the islands, while the steps keep their ratios.

Against the old gradient (white to `zinc-100` light, `zinc-800` to `zinc-900` dark), the light model rows equal the old bottom's and lose a little at the old top (the default body 2.62 to 2.38, the selected body 3.21 to 2.92), and the dark ones gain (the default body 5.67 to 6.75 then, 7.58 now). The default body keeps `zinc-400`, the `body` value io.md uses. Against the viewport every body swatch's base color, or where that is lower the edge color chosen for it (2.16.4), measures at least 4.33:1 (the lowest is the light Red, `red-600`, by its base color), so its faces and B-rep edges show; the White swatch, which matches the light viewport (1.00), shows through `edgeDark` (16.11) at its edges. Curved outlines (the side of a cylinder, a fillet, a sphere) have no B-rep edge, and there the shading of 3.12 pass 2 carries the outline: at a normal across the view (`n.z` 0, `k` 0.32 to 0.66 plus the 0.06 rim) the White swatch's outline reads 1.36 to 2.37 against the light viewport and the Black swatch's 2.20 to 2.35 against the dark one, while the default body's reads 2.81 to 4.29 and 4.21 to 6.43. The light grid keeps `zinc-300` and the old bottom's values; the dark grid moves to `zinc-700`, a little quieter than before (minor 1.24 to 1.27, major 1.56 to 1.75 on the old gradient), where `zinc-600` would reach 1.27 and 1.82 on `zinc-950`. The light origin plane outlines (2.45 to 2.82) and the datum plane outlines in both themes (1.63 and 2.15) stay under 3:1, as on the old gradient: origin planes are named by their plane labels and datum planes by their browser rows, and hovered and selected outlines of both are drawn 2.5 pixels wide in `highlight` (3.12). The analysis ends color faces, whose outline the edges carry, and the legend names them (3.17).

#### 2.16.7 Type

One face sets words, units and numbers, and a monospace face sets code only (the owner's choice of 2026-10-15, option A of the font study's revised brief, ADR 0031). Three roles:

| Role | Face | Features | How it is set | Where |
| --- | --- | --- | --- | --- |
| Interface | Geist 1.800, variable, `wght` 100 to 900 | `ss05`, the serifed capital I, so I never passes for l, 1 or \| | Inherited from the root element, which Tailwind's preflight sets from `--font-sans` and `--font-sans--font-feature-settings` (2.16.1); inputs, buttons and selects inherit both (`font: inherit` and `font-feature-settings: inherit` in the preflight). No class | All interface text: labels, headings, buttons, menus, tooltips, messages, shortcut hints, the names the user types (bodies, features, files, comments), units, and every number |
| Numbers | Geist, as Interface | `ss05`, plus tabular figures where digits line up | `tabular-nums` (`font-variant-numeric`) on the element that holds the number where the table below says tabular; nothing where it says proportional | The table below |
| Code | JetBrains Mono 2.304, variable, `wght` 100 to 800, with `size-adjust: 96%` in its `@font-face`, which brings its x-height (0.550 em) level with Geist's (0.530 em) | `normal`: its dotted zero stays, since code mixes letters and digits | `font-mono`, which sets `--font-mono` and `--font-mono--font-feature-settings` (`normal`, so code drops `ss05`); editable code adds `[font-variant-ligatures:no-contextual]`, so contextual alternates never reshape the characters around the caret | Parameter names and expressions shown as code: the Parameters panel's name and expression fields (editable, 3.11), and the "{name} = {expr}" of the palette's parameter rows (3.4) and of the browser's parameter rows (3.10); the unsupported field's JSON (3.7). When they come (1.2), the Python export view and technical details shown as text (error details, ids) |

Rules:

- Set once: the interface face and `ss05` come from the root element, so no component writes `font-sans` or an arbitrary `font-[...]` family, `roles.ts` has no font classes, and the unsupported page and the app root inherit the face like everything else. The families are named only in the `@font-face` rules and the `@theme` block of 2.16.1; layout, island and type rules (3.2, 3.23) name the roles (interface, numbers, code), never a family, so a change of family edits 2.16.1, this section, `public/fonts` and the tests that check the files and the loaded fonts (U-202, U-203, E-62, E-143, E-144, 6.6), and no component.
- Features never go through `font-feature-settings` in a class or attribute: the property is one list, so a class that set `"tnum"` would replace the root's `"ss05"`. Tabular figures come from `font-variant-numeric` (`tabular-nums`), which the browser applies under the element's `font-feature-settings`, so both hold. No other numeric variant is used (no `slashed-zero`, `ordinal`, `lining-nums`, `oldstyle-nums` or fraction utility, and no `zero` feature), so every number shows Geist's plain zero; the dotted zero of JetBrains Mono appears only in code.
- A field never switches faces: inspector fields, the section offset, the selection inspector's size and dimension fields, the draw inputs, the dimension editor and the color popover's hex field are Interface with `tabular-nums` even while they hold an expression or a parameter name, and so are the name suggestions under them (2.13). Only the Parameters panel's name and expression fields are Code, whatever they hold.
- Units are Interface: a unit the UI writes after a number (`formatQuantity`, a field's unit, a readout's unit, `°`, `³`) is set in Geist beside its number, never in the code face. An expression shown as code is set whole in the code face, unit words included, because it is the text as typed and the UI never splits expressions (only the core parses them, 1).
- Titles (panel titles and `selection-title`, 3.23) are Interface at `font-semibold`; there is no display face.
- Glyphs Geist does not have come from the next family of `--font-sans` (`ui-sans-serif, system-ui, sans-serif`), one character at a time: the macOS key glyphs `⌘`, `⌥`, `⌃`, `⌦` and `⌫` of `formatShortcut` (shown on macOS only, whose system font has them) and the diameter sign `⌀` of dimension labels and printer details (in neither font, so it comes from the system font on every platform, as it did before).
- The UI renders no `kbd`, `samp` or `pre` element, which Tailwind's preflight would set in the code face (the frontend guideline's avoid list names monospace labels); a `code` element carries `font-mono` itself.

Numbers, surface by surface (tabular: `tabular-nums` on the element named; proportional: no numeric class). Tabular figures are for numbers that stand in fields, readouts, columns and counters, where a changing digit must not move what follows it; proportional figures are for numbers read once inside words.

| Surface | Element | Figures |
| --- | --- | --- |
| Expression fields: every `ExpressionInput` without `code` (command inspector fields, the section offset, `entity-size`, the selection inspector's dimension field; 2.13, 3.7, 3.8, 3.15) | the input | tabular |
| Number fields: every `NumberField`, the inspector field of a plain `integer` or `number` param (2.12, 3.7) | the input | tabular |
| Expression results | `expression-result` | tabular |
| Name suggestions | each option of `expression-suggestions` | tabular, or code under a `code` field |
| Selection fields | the text of `selection-field` ("4 edges") | tabular |
| Color popover | the hex field | tabular |
| Coordinate readouts | each number of `body-size`, `body-center` and `vertex-position` (their axis letters are labels, 3.8) | tabular |
| Measurements | the value of each `measure-row`, and the area and length values of the selection inspector | tabular |
| Parameters panel | `parameter-value` (the name and expression fields are Code, the comment field Interface) | tabular |
| Browser parameter rows | the evaluated value (the "{name} = {expr}" text is Code) | tabular |
| Sketch dimension labels | each `sketch-dimension`, and the label that follows the pointer while a dimension is placed (3.16.7) | tabular |
| Draw inputs and dimension editor | each `draw-input` (its value and its measured placeholder) and the input of `dimension-editor` | tabular |
| Repair markers | the number of each `repair-candidate` chip | tabular |
| Timeline | the collapsed timeline's "{n} features" and `timeline-problems` | tabular |
| Status bar | `status-dof`, `status-grid`, `status-recompute` and `health-badge` | tabular |
| Print | the count of the Print toolbar's "Print check" button, the detail of each printer menu item, and the two labels of `overhang-legend` | tabular |
| Titles and labels | `selection-title` ("3 edges"), panel titles, button and menu labels (UI scale items, `keep-sizes`, each `repair-candidate-button`, the printer buttons, the Notes count of the Problems panel), shortcut hints, palette option titles other than parameter rows | proportional |
| Sentences | messages and their cause lines (4.3 and core messages), `field-error`, `field-limit`, hints, tooltips and descriptions, the sketch header's status text, the overhang legend's caption, `export-status` lines and empty states | proportional |

Files (`ui/public/fonts/`, served at `/fonts/`; Vite copies them to `dist/fonts/`, so the web build and the desktop bundle carry them):

| File | Bytes | SHA-256 | Source |
| --- | --- | --- | --- |
| `Geist-Variable.woff2` | 69,652 | `a369fcf5628ea2aa4e1b9e2ec6a5b3624e365bda588e1f0f2f12b564f728fbb8` | `dist/fonts/geist-sans/Geist-Variable.woff2` of the npm package `geist` 1.7.2 (Geist 1.800), unmodified |
| `Geist-OFL.txt` | 4,368 | `930853ee1daa68554d9e35c8a9175affb74f699fad9a5da6ee5ebe76379d9137` | `LICENSE.txt` of that package, unmodified |
| `JetBrainsMono-Variable.woff2` | 56,316 | `4b7bb5bad76ad8b93fb40d378f76184e84f3f08b504b7034bb14f6eb92ac3f46` | `fonts/variable/JetBrainsMono[wght].ttf` of the release `JetBrainsMono-2.304.zip` (the TTF's SHA-256 is `662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016`), converted to WOFF2 and subset as below |
| `JetBrainsMono-OFL.txt` | 4,399 | `30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad` | `OFL.txt` of that release, unmodified |

- Both fonts are under the SIL Open Font License 1.1 with no Reserved Font Name, so a subset keeps its name; each license text ships next to its font, and the third-party notices list both (m1-gap-list B-2).
- Geist is shipped whole. It sets every name the user types, and a Latin and Latin Extended subset (39,948 bytes) would drop its Cyrillic and Vietnamese letters, which would then fall back to the system font in the middle of a name.
- JetBrains Mono sets code only (identifiers of ASCII letters, digits and `_`, expressions with `°`, JSON), so it is subset to Google Fonts' `latin` and `latin-ext` ranges with every layout feature kept: 427 characters, every one of its features that acts on them (34; the one left out, `cv99`, only changes the Cyrillic С and с), its `wght` axis from 100 to 800, every name record and its `prep` and `gasp` tables. The whole font as WOFF2 would be 113,316 bytes. The file is the output of this command, run with fontTools 4.66.1 and the Python `brotli` module 1.2.0 (other versions may write other bytes, which U-202 would catch):

```sh
python -m fontTools.subset 'JetBrainsMono[wght].ttf' \
  --output-file=JetBrainsMono-Variable.woff2 --flavor=woff2 \
  --layout-features='*' --name-IDs='*' --name-languages='*' --notdef-outline \
  --unicodes='U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD,U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+1D00-1DBF,U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF'
```

Loading:

- The two `@font-face` rules of 2.16.1 use `font-display: swap`, so text shows at once in the fallback and takes its face when the file arrives.
- `ui/index.html` preloads Geist, the face of the first paint, with one element in its `<head>`: `<link rel="preload" href="/fonts/Geist-Variable.woff2" as="font" type="font/woff2" crossorigin />`. The URL is the one of its `@font-face`, and `crossorigin` gives the preload the CORS mode Chromium and Firefox fetch fonts in, so the preload is the face's only request (U-203, E-144). WebKit is to reuse the preload for its `@font-face` load too; on the first platforms run it fetched the font a second time while Vite's servers sent `Vary: Origin` with every response, which they no longer send (delivery.md 2.7, open issue 41).
- JetBrains Mono is not preloaded: the browser fetches it the first time code text is laid out, so a session that never shows code never downloads it.
- Every font request goes to the app's own origin. Same-origin responses pass `Cross-Origin-Embedder-Policy: require-corp` without a `Cross-Origin-Resource-Policy` header, the desktop app looks the same offline as online, the desktop content security policy names no remote host (`font-src 'self'`, and `style-src` without a font host, desktop.md 2.8), and nothing is requested from `fonts.googleapis.com` or `fonts.gstatic.com` (E-144).
- Both variable files are unhinted (no glyph instructions, only `prep` and `gasp`). Chromium on Linux renders them as the font study shows; Windows at 12 and 13 CSS pixels and 100 percent scaling is unchecked (open issue 33).

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

- The root element is `document.documentElement`. Before the first render `main.tsx` adds `ROOT_CLASSES` to it and applies the resolved theme and the UI scale; `index.html` declares `<meta name="color-scheme" content="light dark" />` in its `<head>`, so the browser paints the page before the bundle runs (its background and default colors) in the OS's scheme, and a dark OS sees no light flash at cold start; from the first render on, `ROOT_CLASSES` set the root's `color-scheme` to the resolved theme, so an explicit theme overrides the meta (U-163).
- Default: `settings.theme` is `system`, which follows `matchMedia('(prefers-color-scheme: dark)')`, including changes while the app runs (a `change` listener).
- Toggle: the top bar's "Dark theme" button (3.2) and the palette command "Toggle dark theme" set `settings.theme` to `toggledTheme(resolved)`. The Document menu's Theme submenu and the palette commands "Theme: System", "Theme: Light" and "Theme: Dark" choose any of the three; System returns to following the OS.
- Persistence: every settings change is saved at once with `saveSettings` under the localStorage key `partsmith.settings.v1` (2.15), so the choice survives reloads. When localStorage fails, the choice stays in effect for the session.
- A change of the resolved theme re-renders the viewport with `viewportTheme(theme)`.

#### 2.16.9 UI scale

`UI_SCALE_CLASSES` maps `1` to `text-[100%]`, `1.25` to `text-[125%]`, `1.5` to `text-[150%]`, `2` to `text-[200%]` and `3` to `text-[300%]`; `applyUiScale` puts the class on `<html>`, setting the root font size as a percentage of the browser default. These are arbitrary values because Tailwind has no root font-size utility, and they are percentages, never pixels. All chrome sizes are Tailwind rem classes, so they follow the scale; container queries in rem (the app root's `@5xl/app`, the top-center slot's `@xl/slot` and the top bar's `@7xl`, 3.2) follow it too, while media query breakpoints do not (CSS evaluates rem in media queries against the initial font size). Viewport quantities this spec gives in CSS pixels (line widths, pick radii, snap distances, overlay offsets) are multiplied by the scale in code.

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
export type OriginPlane = 'XY' | 'XZ' | 'YZ'
/** The standard view that looks at an origin plane from its front (2.11): "Top", "Front" or "Right". */
export function originPlaneView(plane: OriginPlane): 'Top' | 'Front' | 'Right'
/** "Top plane (XY)", "Front plane (XZ)" or "Right plane (YZ)" (3.10). */
export function originPlaneName(plane: OriginPlane): string
```

`originItem` (`src/inspector/selection-values.ts`) is the one place that builds origin selection items, so browser rows, selection titles, field boxes, Choose menus and scene picks all take their plane labels from `originPlaneName`; the plane buttons of 3.16.1 read `originPlaneView` and the plane labels "{view} ({plane})".

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
| `ps:drop-check` | measure | Timeline drag pointer move | `dropWarning` of the nearest gap returned (3.9) |
| `ps:apply-events` | measure | `applyCoreEvent` (`src/state/core-events.ts`) called with a `document_changed` | `applyCoreEvent` returns for the `recomputed` of the same document when that is the next event it receives |
| `ps:expression` | measure | `useExpression` (2.13) submits a text other than the last one it submitted | The field renders that text's result (its state leaves `pending`) |

Each measure name keeps at most the last 500 entries (`performance.clearMeasures` drops older ones).

`ps:frame` is recorded by the viewport controller for a frame that renders while the camera moves (its camera differs from the previous rendered frame's) or a drag runs (a press past the gesture threshold, 3.13), when the animation frame the controller ran before it also rendered so. It runs between the two frames' `requestAnimationFrame` timestamps, which fall on display refreshes, so a frame that keeps up measures one refresh interval, however late in the frame its callback started. After such a frame the controller asks for the next animation frame: one with nothing to draw, or one that renders while nothing moves (a hover highlight), ends the motion, and the next moving frame records none, so a pause between pointer moves is never timed. A frame rendered outside an animation frame, for a pick, takes `performance.now()` as its time.

`ps:apply-events` spans everything that runs between its two ends: both store updates, the store subscribers they call and the app's other handlers of the `document_changed`. It is recorded for every document, shown or not adopted yet; a `document_changed` followed by any other event, or by the `recomputed` of another document, records none. `ps:expression` starts in the task of the keystroke that changed the text (React runs the effects of a discrete input event before that task ends), or for the first text of a newly shown field. A new evaluation of the same text at a newer revision starts none, and neither does a text whose result the field already shows when it is submitted (a text changed back to one whose result is still kept); a text replaced before its result shows (latest-only, 2.13) records none.

## 3. Behavior

### 3.1 Startup

1. `main.tsx` checks capabilities before anything else: `typeof WebAssembly === 'object'` and a non-null `document.createElement('canvas').getContext('webgl2')`. If either fails it renders only the unsupported page (`data-testid="unsupported"`, with the interface font and the `surface` and `text` roles of 2.16) with the message from 4.3 and starts nothing else.
2. It loads settings, adds `ROOT_CLASSES` to `<html>` and applies the resolved theme (the `dark` class) and the UI scale class to it (2.16.8, 2.16.9), creates the transport with `chooseTransport(window)`, the file service with `chooseFileService(window)` (desktop.md 3.7), the stores and services, calls `createApp`, installs the shortcut dispatcher and the `beforeunload` guard (3.19), on the desktop (`tauriGlobalOf(window)` not null) installs the close guard (`installCloseGuard`, desktop.md 3.6), and renders `App`.
3. The shell renders at once. The viewport shows the loading overlay (`data-testid="core-overlay"`, text "Loading the geometry core") while `app.store.getState().core.status` is `loading`. Sketch mode's overlay (`SketchOverlay` with everything only it uses: the sketch header and hint and the sketch layer, 3.16) is a chunk of its own, so it is not initial JavaScript (5, delivery.md 2.6) and startup never waits for it: `loadSketchOverlay` of `src/sketch/overlay-chunk.ts` loads it once with a dynamic `import()`, which the shell's first render starts, and `App` renders the overlay inside a `Suspense` without fallback, at once when `loadedSketchOverlay` has it, else once it has loaded. Until then sketch mode shows no sketch header or layer (the context switch and the toolbar show it already); a chunk that fails to load is a crash of the viewport region (4.4).
4. `startCore` runs `transport.start()`, then `featureCatalog()` and `listPrinterProfiles()` in parallel, registers the feature commands, and sets the core status to `ready`. A failure sets it to `failed` and shows the fatal state (4.4).
5. It lists the recovery entries that no running page holds (2.15) and offers them one at a time (3.19); the app does not open them automatically.
6. It opens a new document (`newDocument('mm')`) named "Untitled". The `app` root element gets `data-core-status="ready"` only after this document's first `document_changed` is applied.
7. When the first viewport frame renders with that document, `ps:usable` is marked.

### 3.2 Layout regions

The window is a fixed layout with no page scroll. The 3D view fills the whole window, and the other regions float over it as islands: rounded cards with a gap from the window's edges and from each other (the Partsmith direction in [docs/guidelines/README.md](../guidelines/README.md), ADR 0030). The app root element (`data-testid="app"`) is `@container/app relative grid h-dvh w-dvw grid-cols-[auto_minmax(0,1fr)_auto] grid-rows-[auto_minmax(0,1fr)_auto_auto] overflow-hidden p-2` with `stage` and `text` (2.16) and `antialiased`, in the interface face it inherits from the root element (2.16.7); `stage` is the viewport's color, so before the first frame, while the WebGL context is lost and when the viewport region has crashed, the gutter still reads as the 3D view behind the islands, not as one surface with them. Each region places itself in that grid (top bar, timeline and status bar `col-span-3` in rows 1, 3 and 4; browser, viewport and side panel in columns 1, 2 and 3 of row 2), so the DOM keeps the tab order of 3.24: top bar, browser, viewport, timeline, side panel, status bar, notifications. The grid has no gap: each island keeps 0.5rem from the free area with a margin on the side that faces it (top bar `mb-2`, browser `mr-2`, side panel `ml-2`, timeline and status bar `mt-2`), and `p-2` keeps 0.5rem from the window's edges, so a closed island leaves no double gap. Sizes are Tailwind classes on the rem scale (at 100 percent UI scale, 1rem is 16 CSS pixels):

| Region | Element | Size | Content |
| --- | --- | --- | --- |
| Top bar | `<header>`, `data-testid="top-bar"` | full width inside the gutter, `h-12` | Browser toggle, document name and save state, context switcher, context toolbar, palette button, Export button, theme toggle |
| Browser | `<nav aria-label="Browser">`, `data-testid="browser"` | `w-66`, left | Tree (3.10) |
| Viewport | `<main aria-label="3D view">`, `data-testid="viewport"` | the free area: the grid cell the islands leave; its canvas fills the window | Canvas and overlay layer (under the islands), free-area layer with the view cube, viewport controls, top-center slot (empty state, sketch header and hint, print hint), legends and inspector |
| Side panel | `<aside>`, `data-testid="side-panel"` | `w-90`, right, only while open | Parameters, Print check, Problems or Keyboard and mouse (one at a time) |
| Timeline | `<section aria-label="Timeline">`, `data-testid="timeline"` | full width inside the gutter, `h-14` (`h-7` collapsed) | Timeline (3.9) |
| Status bar | `<footer>`, `data-testid="status-bar"` | full width inside the gutter, `h-7` | Status items (3.21) |
| Notifications | the region after the status bar, in the free area's grid cell (`col-start-2 row-start-2`); the palette layer, the live regions and the drop overlay follow it | over the free area | Notification stack (3.22) |

Islands and cards, by tier (the visual language of shadcn/ui; colors are roles of 2.16.3, never palette steps):

| Tier | Elements | Shape and depth | Surface |
| --- | --- | --- | --- |
| Frame islands | Top bar, browser, side panel, timeline, status bar; a crashed region's fallback (4.4) in its place, which takes its region's island classes and margin through the `fallbackClassName` that `App.tsx` passes and, like every island, scrolls only its body; in the top bar, timeline and status bar (`bar`) that body is one row whose message truncates before a compact Reload (`Button` `size="compact"`, `h-6`), so both fit the status bar's 1.75rem | `relative z-10 rounded-xl border shadow-md`; the status bar and the collapsed timeline, under 2.5rem tall, `rounded-lg`, so a slim bar never reads as a pill; the browser and side panel as overlay islands `shadow-lg` on `surfaceRaised` (below) | `surface` background (the overlay islands also `surfaceRaised`'s `overlay` facet), `forcedEdge` border, `elevation` shadow |
| Cards | Multi-line cards, `rounded-xl border`: the inspector, the core panels of 4.4 and the context-loss panel of 3.12 (one component), which come and go over the model, `shadow-lg`; notifications and the drop overlay (3.19, which keeps its dashed `accent` border), which come and go over the islands too, `shadow-lg` on `surfaceRaised`; the viewport controls, the overhang legend and the empty-state card, which stay, `shadow-md`. The sketch dimension editor (3.16.7), `rounded-lg border shadow-lg`, which comes and goes over the sketch in the overlay layer, under the islands. One-line chips, `rounded-lg border`: the sketch header, the sketch hint and the print hint, which stay while their step lasts, `shadow-md`, and the timeline's drop hint, which rises over the free area during a drag (3.9), `shadow-lg`. The view cube's Home view button, `rounded-md border shadow-md`, which stays (3.14) | as listed | `surface` (notifications and the drop overlay `surfaceRaised`), `forcedEdge`, `elevation` |
| Popups | Menus, popovers, tooltips and the `expression-suggestions` list (`rounded-lg border shadow-lg`); the palette (`rounded-xl border shadow-lg`) | as listed | `surfaceRaised`, `forcedEdge`, `elevation` |
| Inside cards | Status banners (`inspector-error`) `rounded-lg`; the read-only JSON block of an unsupported field (2.12) `rounded-md border`, the shape of an input | as listed | banners their status roles, their buttons on the `banner` facets of `tonal` and `hover` (3.23); the JSON block `tonal` with a `forcedEdge` border |
| Controls | Buttons, icon buttons, inputs, selects, selection boxes, segmented control tracks, switches, menu items, palette options, browser rows, timeline items and the timeline's drag ghost, the sketch draw inputs (no shadow) | `rounded-md` (a segment inside its track and a switch's thumb `rounded-sm`); never `rounded-full` on a button | their roles (3.23): tonal and ghost buttons, the segmented control's `tonal` track and `thumb`, and a `borderControl` boundary on inputs, selects, selection boxes and switches |
| Marks | Plane and axis labels, constraint glyphs, dimension labels, the draw inputs' readout chip, repair candidate markers, swatches | `rounded-sm`, no shadow (the legend's bar and text-sized inline buttons keep `rounded-xs`) | their roles as before |

Island content: an island never scrolls as a whole. It is `flex flex-col overflow-hidden`, and an inner body scrolls (`min-h-0 flex-1 overflow-y-auto`: the browser's tree, a side panel's body), so no scrollbar runs into the rounded corners. The timeline is the one island that does not clip, so its drop hint and drag ghost can rise above it (3.9). Lists inside an island (the tree, the Problems list) are inset with `p-1`, so their `rounded-md` hover and selection backgrounds stay inside the island's corners. Island headers (`PanelHeader`) keep `h-12 px-4` with the title in `text-base font-semibold` and no rule under them, like the print check panel's header block: the title's weight and the body's padding set them apart. Lines inside islands, cards and popups are as few as possible: spacing and tonal grouping set rows and regions apart, and a `divider` hairline (2.16.3) stays only where they cannot. These stay: menu separators (the `hr` between groups of items in every `Menu` and in the printer menu), since the last item of one group and the first of the next would otherwise read as one list; the rule under the palette's search field (`border-b` of its header row), since the options scroll up under it; and the edge of a scrolling body while content is scrolled under a fixed header or continues under a fixed footer, since cut-off rows would otherwise run into the title or the buttons: the top rule (`border-t`) of the side panels' bodies under `PanelHeader` (Parameters, Problems, Keyboard and mouse), of the print check panel's list under its header block and of both inspectors' bodies under their header, and the bottom rule (`border-b`) of the command inspector's body above its footer (so its body is `border-y`) and of the palette's list above `palette-footer`. Such a body keeps its rule in `forcedEdge` at rest and takes `divider`'s `overflowTop` or `overflowBottom` facet, which colors the rule while `useOverflowEdges` (`src/components/overflow-edges.ts`) marks the body `data-overflow-top` (its `scrollTop` is above 0) or `data-overflow-bottom` (its `scrollTop` plus `clientHeight` falls more than 1 pixel short of its `scrollHeight`). The hook sets the attributes on the element itself, never through React state, so scrolling renders nothing, and measures again on `scroll` (a passive listener), on a `ResizeObserver` of the body and of its element children, and when its children change (a `MutationObserver` of its child list, which observes the new children). Every other rule is gone: panel header rules at rest, the palette's footer rule at rest, the inspector footer's rule at rest (the footer keeps its `px-4 py-3`), the rules between parameter rows (each keeps `py-3`, 1.5rem between rows against 0.5rem inside one), the rules above the Keyboard and mouse panel's sections (each keeps `pt-4` under its semibold heading), the rule under the print check panel's header block at rest, the line of toolbar separators (each stays an `aria-orientation="vertical"` `hr`, `mx-1 h-6 w-0 shrink-0 border-0`, a gap without a line) and the sketch header's rule between its label and its status (`gap-3` and the status's `textMuted` set them apart).

Depth: no island, card, popup or chip has an outline. Each is a tonal step lighter than what it floats over, plus one faint shadow in the `elevation` role: `shadow-md` for what stays (frame islands, and the cards and chips that rest over the viewport) and `shadow-lg` for what comes and goes over them (overlay islands, the inspector, notifications, core panels, the palette, menus, popovers, tooltips, the dimension editor, the timeline's drop hint, the drop overlay). Islands, cards and chips are `surface` (white in the light theme, `zinc-900` in the dark theme) over the flat `viewport` (`zinc-100` and `zinc-950`, 2.16.4), 1.09:1 and 1.12:1 at every point, so every island, the timeline and status bar included, shows where it ends and the pickable gaps beside it (3.13) can be seen; the light step is the largest the accent allows (a darker light viewport would put the accent's marks under 3:1, 2.16.4, ADR 0032). What floats over the islands themselves is `surfaceRaised`: menus, popovers, tooltips, the `expression-suggestions` list, the palette, notifications and the drop overlay, `zinc-800` in the dark theme (1.18:1 over an island, 1.33:1 over the viewport) and white in the light theme, where the white islands leave no lighter step. The browser and the side panel take it too while they float over the free area as overlay islands (under 65rem, `surfaceRaised`'s `overlay` facet), since they come and go over its cards and chips (the viewport controls, the overhang legend, the slot's cards and chips, the inspector), which would otherwise meet them at one step. The shadows fall below and beside an element, never above it (2.16.6): in the light theme they mark an island's lower and side edges on the viewport (about 1.42:1 below and 1.20:1 beside), while its top edge shows by the step alone, and a white popup over a white island shows by its shadow alone (about 1.32:1 below, 1.09:1 beside, nothing above); in the dark theme no shadow shows on the near-black viewport, so the step alone sets the islands apart, and the shadow, black at 20 percent, shows under popups and over the model as soft depth (a body darkens by at most about 1.6:1 just under a lower edge). Shadow lengths are CSS pixels, so at 200 and 300 percent UI scale they reach half and a third as far relative to the islands, while the steps hold. Surfaces of one step still meet only where something transient lies over another raised surface, set apart by the shadow alone (in the dark theme about 1.03:1 beside and 1.11:1 below it): a submenu beside its parent menu and, under 65rem, a menu, popover or tooltip opened from an overlay island, or a notification over the side panel floating there. Inside an island depth stays tonal (3.23): the `hover` and `tonal` fills, translucent in the dark theme so they read on `surface` and `surfaceRaised` alike, fields on `field` with a `borderControl` boundary (white on the white light islands, `zinc-950` set into the dark ones) and a segmented control's raised `thumb` on its `tonal` track. Each island, card, popup and chip keeps the 1-pixel `border` its outline had, in `forcedEdge` (`border-transparent`): nothing draws it and no box changes size, and in forced-colors mode, where shadows and backgrounds vanish, the system paints it as the edge (3.24). The chrome recedes through quiet color, size and weight, not darkness (2.16.4). Islands are opaque, with no backdrop blur.

Stacking, bottom to top: the canvas, the overlay layer, the sketch layer and the free-area layer, all z-index auto in DOM order; frame islands `z-10`; overlay islands `z-20`; the notifications region `relative z-30`, so an overlay island never hides an error and its actions; the palette layer and the drop overlay `z-50`; menus, popovers, tooltips and the `expression-suggestions` list (2.13) in the top layer. The free-area layer is `isolate`, so the z-indexes inside it (the inspector `z-20` over the slot `z-10`, the view cube's Home button `z-10`) order its own children only and stay below every island.

Free area: the viewport element (`main`, the grid cell of column 2, row 2) is the free area, the part of the window no island covers, and its box is what tests measure as the view (6.3). `main` itself is not positioned, so the layers that work in canvas pixels take the app root as their containing block and fill the whole window under the islands: the element holding the canvas, the overlay layer of 3.12, a canvas-aligned host (`pointer-events-none absolute inset-0`) that the viewport provides through React context (`useCanvasLayer()`), into which `SketchOverlay` portals its sketch layer (3.16.2) while it renders the sketch header in the slot, and the viewport menu's point anchor (3.13). Their CSS pixels are canvas pixels. The rest of the viewport's chrome sits in the free-area layer, the last child of `main` (`pointer-events-none relative isolate size-full rounded-xl`): the view cube (3.14), the left column with the viewport controls and the overhang legend (`ViewportProps.legend`, which `App` fills with `PrintLegend`; 3.14, 3.17), the top-center slot, the inspector (3.7) and the core's loading, fatal and still-working panels and the context-loss panel (4.4, 3.12; their layer over the canvas is inside this one, so they center in the free area). `pointer-events` is inherited, even into the top layer, so the interactive elements of the free-area layer carry `pointer-events-auto`: the view cube's fieldset, the controls' card, the slot's cards and chips, the inspector and the core panels' cards; the shared `Menu` and `Popover` carry it on their popups, so the selection filter menu and the section popover work inside the layer.

- Framing: fit, the standard views, Home, Look at and zoom center the model in the free area through an off-center projection (2.11), not a smaller canvas, so the model shows through the gaps between islands and slides under them when panned. While the inspector is open, Fit to view, Home, Look at and the Show buttons frame into the part of the free area beside it (2.11), since the inspector is an island too (ADR 0030); opening, moving or closing it moves no camera.
- Picking: islands sit above the canvas, so a pointer over an island never reaches it: nothing is hovered, picked, selected or zoomed through an island, and moving onto one clears the hover (3.13). The gaps between islands and the window gutter are canvas and pick as usual. Overlay elements anchored outside the free area are hidden (3.12), so no focusable glyph, label or input sits under an island.
- Giving the space back: opening, closing or collapsing a docked island resizes the free area. A camera that a fit set and nothing has moved since frames the same bounds again in the new area, so a model fitted to view stays whole; any other camera keeps its target, rotation and visible height (2.11), so the model moves to the new free area's center at once (no animation, as a resize) and keeps its size when only the width changes.

Top bar, left to right:

- Browser toggle: icon button "Toggle browser" (`panel-left`), `aria-pressed` reflecting the browser state, shown pressed by `hover`'s `active` fill rather than `selection` (3.23).
- Document name: a button (`data-testid="document-name"`, text the document name, `data-dirty="true|false"`) that opens the Document menu; with its save state it keeps at least 5rem (`min-w-20`) and the name truncates inside it with an ellipsis, so it never collapses under the context switcher. Next to it, `data-testid="save-state"` shows "Edited" in `textMuted` while the document is dirty (core `dirty` or `localUnsaved`) and is empty otherwise.
- Context switcher: `role="radiogroup"` with `aria-label="Context"` and three radios "Model", "Sketch", "Print" (segmented control, one tab stop, arrow keys move).
- Context toolbar: `role="toolbar"` with `aria-label` "Model tools", "Sketch tools" or "Print tools" (3.3).
- Palette button: "Search" with the formatted `Mod+K` hint (`data-testid="palette-button"`, the hint from `@3xl`), and the tooltip "Search" with that hint. In a top bar under 48rem (`@max-3xl`, as at 200 percent on a 1280 CSS-pixel window) it and the Export button show their icon only: their labels are `sr-only`, so they stay the accessible names, and their tooltips name them.
- Export button: "Export" (`download` icon) opening the export popover (3.18).
- Theme toggle (`data-testid="theme-toggle"`): icon button "Dark theme" (`moon` icon), `aria-pressed` true while the resolved theme is dark and shown pressed like the browser toggle; it switches to the opposite theme and persists the choice (2.16.8).

Document menu (`role="menu"`, `data-testid="document-menu"`), in order: New (desktop shortcut shown), Open..., Open recent (submenu, 3.19), Save, Save as..., Export..., separator, Theme (submenu of radio items System, Light, Dark), Navigation (submenu: Fusion, FreeCAD), UI scale (submenu: 100%, 125%, 150%, 200%, 300%).

Responsive rules:

- Docking: while the app root's content box is at least 64rem wide (`@5xl/app`; a container query measures the content box, inside the root's `p-2`, so this is a window of at least 65rem: 1040 CSS pixels at 100 percent, 2080 at 200 percent, following the UI scale), the browser and side panel dock: they take their grid columns and narrow the free area. In a narrower window they float over the free area as overlay islands: `@max-5xl/app:absolute @max-5xl/app:inset-y-0 @max-5xl/app:z-20 @max-5xl/app:shadow-lg @max-5xl/app:col-start-2 @max-5xl/app:col-end-3 @max-5xl/app:row-end-3` with `surfaceRaised`'s `overlay` facet (2.16.3), the browser `@max-5xl/app:left-0` and the side panel `@max-5xl/app:right-0`, both `max-w-full`. An absolutely placed grid item takes its grid area as its containing block, and an `auto` line would be the root's padding edge, so both name the free area's cell on every side: they span the free area's height, keep its gaps to the top bar and the timeline, and lie against its sides. They leave the free area unchanged and close on Esc or on a pointer press outside them. The code that closes them tests the same condition, `window.innerWidth` below 65 times the root font size (the app root spans the window), since a media query evaluates rem against the initial font size and would miss the UI scale. `createAppStore` (not the constant `INITIAL_STATE`, 2.8) starts the browser closed when `window.innerWidth` is below 65rem at the stored UI scale: 65 times the root's font size before the UI scale class (the browser's default, 16 CSS pixels unless the user changed it; `main.tsx` creates the store before it applies the class) times `settings.uiScale`. So at 200 percent on a 1280 CSS-pixel window the browser starts closed and opens as an overlay.
- At 100 percent on a 1280 by 720 window with the browser open the free area is 992 by 548 CSS pixels at (280, 64); with the Parameters panel open as well, 624 by 548; at 1040 by 720 with both docked, 384 by 548, the narrowest docked free area at 100 percent; at 200 percent on 1280 by 720 (both as overlays), 1248 by 376 at (16, 128).
- The top bar is a container (`@container`). The context toolbar measures, at runtime, the room the top bar leaves it (after each render, on a `ResizeObserver` of the toolbar and once the fonts have loaded; `src/layout/toolbar-fit.ts`, `nextFit`), so it fits any window, document name and UI scale. Its buttons show icon and label (`h-9 px-3`, the height of the icon-only buttons, so the toolbar with its `p-1` is 2.75rem inside the island's 2.875rem) when every labeled button fits; otherwise the primary-style buttons (Finish sketch, Export 3MF) keep their label and the others show the icon only (`size-9`; the label stays the accessible name and appears in the tooltip). When that does not fit, the last buttons that are not primary move, in toolbar order, into a "More tools" menu button (`ellipsis` icon, icon only) at the toolbar's end while the primary buttons keep their label, so the way out of a sketch is named whenever possible; only when a labeled primary button does not fit beside More tools with every other button moved is every button icon only, and as many of the moved buttons as then fit come back. The More tools menu lists them with icon, title and shortcut (a group as a submenu, a toggle as a checkbox item; Choose printer and Checks open the printer menu and the checks popover under More tools). Primary buttons never move. A toolbar whose content changes (context, catalog, printer name, finding count, UI scale) is measured again from every label shown; each narrowing or widening step is drawn only before the browser paints. The toolbar never scrolls (`overflow-hidden`, at least `min-w-28`, so More tools and the primary button stay in reach while the document name truncates), and its `p-1` keeps every focus ring whole (3.23). No command is ever out of sight: it is a button or an entry of More tools.
- The minimum supported window is 800 by 600 CSS pixels at 100 percent and 40rem by 22.5rem at any UI scale (1280 by 720 CSS pixels at 200 percent, 1920 by 1080 at 300 percent).

Floating elements (shapes and depth by the tiers above): the inspector (3.7, 3.8) in the free area; the notifications stack at the free area's bottom-right (3.22); the palette, `w-140 max-w-full max-h-full`, centered in a layer `fixed inset-x-4 top-1/6 bottom-4 flex items-start justify-center` (so it keeps 1rem from the window's sides and bottom at any UI scale; its list, `max-h-96 min-h-0 flex-1 overflow-y-auto`, scrolls inside it); menus, popovers and tooltips anchored to their triggers (native Popover API, light dismiss), and the name suggestions of expression fields anchored to their inputs (2.13), positioned at runtime (3.12), free to overlap islands. The top bar, the timeline and the status bar are marked `data-bar`: a menu, popover or tooltip anchored to one of their controls is placed against the trigger's width and the whole island's height, so it keeps its 0.25rem gap from the island's edge instead of touching or covering it. An anchor inside a popup opened from a bar is not one of its controls: a submenu of the Document menu opens beside its item.

Corners of the free area: the view cube takes the top-right corner and the viewport controls the top-left (3.14); the overhang legend sits at the bottom of the left column under the controls, or right of them in a short free area (3.14, 3.17), and the notifications at the bottom-right (3.22).

Top-center slot: a column at the free area's top that keeps clear of the view cube and the viewport controls at every width. `TopCenterSlot` (`src/viewport/TopCenterSlot.tsx`, used by `EmptyState`, `SketchHeader` and `PrintHint`) renders the slot, `pointer-events-none absolute inset-4 flex flex-col @container/slot`, holding an empty `aria-hidden` spacer `@max-xl/slot:h-44` and then the column (`data-testid="top-center-slot"`), `flex shrink-0 flex-col items-center gap-2 @xl/slot:mx-38`. While the slot is at least 36rem wide, the column starts 1rem below the free area's top and keeps 10.5rem from each side of it (1rem plus `mx-38`), beyond the 9.81rem the turned cube reaches from the free area's right edge at 300 percent UI scale (3.14), 0.5rem clear of the Home view button (8.5 to 10rem from that edge, 3.14), so two chips of one tone never touch, and beyond the 3.5rem of the controls on the left. In a narrower slot (a free area under 38rem, for example a window of 1040 to 1263 CSS pixels at 100 percent, or 2080 to 2527 at 200 percent, with the browser and a side panel both docked) the column spans the slot and starts 12rem below the free area's top (1rem plus the spacer), below the cube's lowest corner (11.01rem at 300 percent) and the controls. The spacer is a flex item that shrinks before the column, so in a free area too short for both the column rises toward the top rather than going under the timeline, over the cube's lower corners if it must (it follows the cube in the DOM). It holds one of: the empty-state card (below); the sketch header (3.16.8) with the sketch hint (3.16.3) under it; or the print hint (3.17). Interactive children are `pointer-events-auto`, so the slot never blocks picking.

Empty state: while `selectEmptyState(state)` holds (a ready core, a document without features, no feature command, no sketch session and no Lay on face pick, whose print hint takes the slot), the slot shows a card (`data-testid="empty-state"`, `role="group"`, `aria-label="First step"`, `surface`, `rounded-xl border` in `forcedEdge`, `shadow-md` with `elevation`, `p-3`, `max-w-140`):

- Line 1 (`text-sm`): "Start with a sketch on a plane, or add a box." followed by the buttons "Sketch" (runs `feature.sketch`) and "Box" (runs `feature.box`; left out when the catalog has no `box`).
- Line 2 (`text-xs`, `textMuted`): `navigationLegend(settings.navigation)` (2.11), a space, and "Search every command: {formatShortcut('Mod+K')}." Each hyphenated word of it, a gesture such as "Shift+right-drag", is a `whitespace-nowrap` span, so the line never breaks inside a gesture.

The card follows the document only: it disappears with the first feature, returns whenever the document has no features again (Delete, Undo, New), and has no first-run flag and no setting, so an experienced user sees it only in an empty document, where it costs nothing. In a document without features the origin axes and point are shown too (3.10), and the ground grid gives scale (3.12).

### 3.3 Contexts and toolbars

`TOOLBARS` (labels are command titles; feature titles are manifest labels):

| Context | Entries in order |
| --- | --- |
| Model | Sketch (`feature.sketch`), Extrude, Revolve, Hole, Fillet, Chamfer, Shell, group "Shapes" (`cuboid` icon: Box, Cylinder, Sphere, Cone, Torus), group "Pattern" (`grid-3x3` icon: Linear pattern, Circular pattern), Mirror, Boolean |
| Sketch | Line, group "Rectangle" (`rectangle-horizontal`: 2-point rectangle, Center rectangle), Circle, Arc, Point, Construction, Dimension, group "Constrain" (`magnet`: the 13 constraint commands in the order of 3.16.6), separator, Finish sketch (primary style) |
| Print | Printer (menu button showing "Printer: {name}"), Overhang map (toggle), Checks (popover), Print check (opens the print check panel; shows the finding count), Lay on face (toggle, 3.17), separator, Export 3MF (primary style), group "Export" (`file-output`: STL, OBJ, STEP) |

A separator shows only between two shown controls, so the Sketch toolbar shows none while a plane is picked and it has no buttons. Datum plane, Move body and Split body are in the palette only; the selection inspector also suggests them for what is selected (3.8). A group button opens a menu (`role="menu"`) of its commands with icon, title and shortcut. Disabled toolbar commands render with `aria-disabled="true"`, keep their place, show the reason in their tooltip, and show it as an info notification when clicked (as their shortcut does, 2.10). A group button whose commands are all disabled is disabled the same way with the first command's reason.

Context rules:

- The app starts in Model. `data-context` on the `app` root mirrors the context.
- Model to Print, or Print to Model: an active feature command is canceled first (`cancel_transaction`). The selection is kept.
- Choosing the Sketch radio (or `context.sketch`) outside sketch mode runs `enterSketch` when the app selection is exactly one sketch feature, or items of one sketch (curves, points, regions) (`selectSketchToEdit`), and `startNewSketch` (3.16.1) otherwise; its tooltip reads "Edit {label}" or "New sketch" (`selectSketchRadioTooltip`, also `context.sketch`'s palette detail). In sketch mode, where choosing it changes nothing, it has no tooltip; the radio keeps its focus either way. The toolbar's Sketch (`feature.sketch`) always starts a new sketch. In sketch mode, choosing Model or Print finishes the sketch (3.16.11) and then switches.
- A feature command run from Sketch finishes the sketch, switches to Model and starts the command. When the app selection is empty, the finished sketch preselects: for a `sketch_region` field, its region when it has exactly one; for a `sketch_point` field, its non-construction point entities, or, when it has none, the centers (`{entity}.center`) of its non-construction circles and arcs. Run from Print, it switches to Model first.
- Print commands are listed and active in Print only. Sketch tools are listed and active in Sketch only, and only while a sketch is being edited (not during plane picking).

### 3.4 Command palette

- Opens with `Mod+K` (from anywhere, including text fields) or `S` (when focus is not editable), or the palette button. `Mod+K` while open closes it.
- Layout: a container `role="dialog"` with `aria-label="Command palette"` and `data-testid="palette"`, holding a `role="combobox"` input (`data-testid="palette-input"`, placeholder "Search commands, features, bodies, parameters and settings") with `aria-controls` pointing at a `role="listbox"`, and a footer. Options (`role="option"`, `data-testid="palette-option"`, `data-item-id`, `data-kind`) show icon, title, detail (`textMuted`), a kind tag for non-commands ("Feature", "Body", "Parameter", "Recent file", `text-xs` in `textMuted`), and the formatted shortcut (`data-testid="palette-shortcut"`, `textMuted`, the interface font) when it is active in the current context (`shortcutsActiveIn`, 2.10), so Hole shows no H in Sketch, where H is Horizontal. The active option has the `hover` role's `active` background.
- Items (`paletteItems`): every registered command whose availability is not `hidden` in the current context; every feature in the document ("{label}", kind feature, runs: select it in the timeline and scroll it into view); every body ("{name}", kind body, runs: select it and frame it); every parameter ("{name} = {expr}", kind parameter, its title in the code face, `font-mono`, 2.16.7; runs: open the Parameters panel with that row's expression input focused); recent files ("Open recent: {name}", kind recent). Settings are ordinary commands of category Settings: "Theme: System", "Theme: Light", "Theme: Dark", "Navigation: Fusion", "Navigation: FreeCAD", "UI scale: 100%" to "UI scale: 300%", each with `checked` showing the current value.
- Ranking: `rankPaletteItems` with `settings.paletteRecent` and the current context (2.10). The first option is active when the list changes.
- With an empty query the options are grouped under headings (`role="group"` with `aria-label` the heading; the heading is `text-xs` in `textMuted`, `data-testid="palette-heading"`, and is not an option): "Recent" when there are recent items, then the category names in the context order of 2.10. So every command can be browsed without knowing its name. Arrow keys move over options only.
- When a non-empty query matches nothing, the listbox shows one row that is not an option (`data-testid="palette-empty"`, `textMuted`): "Nothing matches {query}."
- Keys: ArrowDown and ArrowUp move the active option and wrap; PageDown and PageUp move by 8; Enter runs the active option and closes the palette; Esc closes it. Focus stays in the input. Tab is ignored.
- Mouse: hovering an option makes it active; clicking runs it.
- Footer (`data-testid="palette-footer"`, `text-xs` in `textMuted`): the active option's description (2.10), which the option references with `aria-describedby` (the footer is not a live region, so arrow keys are not announced twice). When the active option has none and no reason is shown, it names the keys instead (`aria-hidden`): "Enter to run, Esc to close", or "Esc to close" with no option, the keys formatted for the platform, so it is never an empty strip. A disabled option is dimmed with its reason as the detail; Enter on it shows the reason in a `role="status"` line of the footer instead and keeps the palette open.
- Running any item records its id at the front of `settings.paletteRecent`; running a feature command from any other source (toolbar, shortcut, menu, suggestion) records it too.
- Closing returns focus to the element that had it before the palette opened. A pointer press outside closes it.
- Each keystroke re-ranks synchronously; `ps:palette` measures it.

### 3.5 Keyboard shortcuts

Registry shortcuts (displayed with `formatShortcut`; `Mod` is Cmd on macOS and Ctrl elsewhere). The last column gives the contexts in which the shortcut works. It equals the command's `contexts` except for Hole, Fillet and Move body: like every feature command they are listed in the palette in all three contexts (2.10, 3.3), but their letters work in Model and Print only (`shortcutContexts`), because Sketch uses H for Horizontal and keeps F and M free.

| Command id | Title | Shortcut | Shortcut contexts |
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

`edit.repeat` runs `startFeatureCommand` for the type of the last feature command run in this session from any source (`lastFeatureType`, 2.8; a new sketch is not repeated, since it starts plane picking), with the current selection preselected, through the registry, so that command's availability applies and its run is recorded as a recent palette item; before any it is disabled with `nothing_to_repeat`. Its `label` (2.10) gives the palette and menus "Repeat {title}". It is reached from the viewport context menu (3.13) and with Enter on the focused viewport (Enter ladder, below).

`FEATURE_SHORTCUTS` holds the four feature entries above with their shortcut contexts; other commands have no shortcut. Letters follow Fusion where Fusion has a default (E, H, F, M, L, R, C, D, X, S); `Mod+N` is desktop only because browsers reserve Ctrl+N. Unit test U-22 asserts `conflicts()` is empty for the built-in registry with the test catalog on every platform. Letters stay shortcuts: typed entry on the viewport routes only characters that start a number (3.7, 3.16.4), so S, V and the feature letters keep working during commands and while a shape is rubber-banding; once a value input has focus, every key types into it.

Keys handled by components, not the registry. The Key, Where and Effect cells are written for users, because the keyboard and mouse panel shows them as they are (`COMPONENT_KEYS`, below); `Mod` is Cmd on macOS and Ctrl elsewhere, and the panel shows the platform's key. The last column names the rule each row follows:

| Key | Where | Effect | Rule |
| --- | --- | --- | --- |
| Esc | Anywhere | Step back one level: close a menu or list, cancel a drawing, leave the tool, cancel the command, then clear the selection | Esc ladder below |
| Enter | Fields, inline editors, the line tool, the view | Commit the field or editor, end a line, confirm a complete command, or repeat the last command on the selection | Enter ladder below |
| Arrow keys | The view | Orbit 15 degrees per press: Left and Right about the vertical axis, Up and Down about the view's horizontal axis | Left and Right about world Z, Up and Down about the camera right axis (3.13) |
| Shift+arrow keys | The view | Pan by a tenth of the view's height | 10 percent of the viewport height |
| = or +, - | The view | Zoom in or out at the center; during a command or while drawing a shape, - starts a negative value instead | By 1.25 at the viewport center; 3.7, 3.16.4 |
| Digit, ., -, ( | The view or a selection field during a command | Type into the command's value field | 3.7 |
| Tab | The view during a command | Select the command's value to type over it | 3.7 |
| Digit, ., -, ( | A sketch while drawing a shape | Type into the shape's first size input | 3.16.4 |
| Arrow keys, Home, End | Toolbars, timeline, browser, menus, segmented controls | Move focus | Roving tab index (3.24) |
| Shift+F10, ContextMenu | Timeline item, browser row, the view | Open the context menu | 3.9, 3.10, 3.13 |
| Space | Timeline item, browser row | Select; with Mod, add or remove | 3.9, 3.10 |
| Tab | Sketch size inputs | Move to the next input | 3.16.4 |
| Hold Mod | A sketch while drawing | Place freely, without snapping or automatic constraints | Suppresses inference (3.16.5) |
| Shift or Mod with click | The view, timeline, browser | Add to or remove from the selection | 3.6 |

Esc ladder: each press does the first step that applies: (1) close the innermost open menu, popover, palette, name completion list, inline rename or inline editor; (2) in sketch mode, cancel the in-progress drawing, drag or box selection (the tool stays active); (3) exit the active sketch tool or constraint pick mode; (4) cancel the active feature command; (5) cancel the plane pick of a new sketch (back to Model) or the face pick of Lay on face (3.17); (6) clear the selection. Esc never leaves sketch mode once a sketch is being edited; when an Esc press in sketch mode finds no step to do, the sketch hint shows `sketch_leave` for 3 s, so the way out is named.

Enter ladder: (1) in an open name completion list with an option chosen by the arrow keys, insert it (2.13); otherwise commit the focused inline editor (rename, dimension editor, draw inputs); (2) in the line tool with a chain in progress and no draw input focused, end the chain; (3) when focus is in the inspector (not on a button) or on the viewport and a feature command is active, confirm it if it is complete (a held key's repeated `keydown` events confirm nothing, and neither do they in an expression field); (4) on the focused viewport in Model or Print with no feature command active and a non-empty app selection, run `edit.repeat` (a held key's repeated `keydown` events, `event.repeat`, are ignored, so holding Enter never chains commands). Enter on a focused button activates that button only.

Keyboard and mouse reference: `app.keys` (F1; the browser's own F1 is prevented) opens the side panel `keys` (`data-testid="keys-panel"`, title "Keyboard and mouse", close button), rendered from data, never hand-written prose, so it cannot drift from the behavior:

- "Mouse": the `rows` of the current preset from `NAVIGATION_PRESETS` (2.11) as label and gesture rows, under a segmented control "Fusion | FreeCAD" that runs the navigation setting commands.
- "Keys in the view and panels": `COMPONENT_KEYS` (`src/commands/component-keys.ts`), the Key, Where and Effect columns of the component-key table above as data (`key`, `place`, `effect`), with `Mod` shown as the platform's key (`Ctrl`, or `⌘` on macOS, as `formatShortcut` writes it).
- "Typing values": `TYPING_KEYS` from the same module, the expression input keys of 2.13 (ArrowUp and ArrowDown step a plain number, Shift ten times as far, Alt a tenth; Tab completes a parameter name), the draw input keys of 3.16.4 and the typed-value keys of 3.7, followed by `TYPING_NOTE`: "Any size or angle field takes a number, a value with a unit such as 1 in, or a formula with parameter names."
- A last line: "Every command and its shortcut: Search ({formatShortcut('Mod+K')})."

Nothing appears until the user asks for it; the palette lists every registry command and shortcut, and this panel covers what the palette cannot: keys components handle themselves and the mouse.

### 3.6 Selection, hover and picking

Pick classes and priority (`choosePick`): `point` (vertex, sketch point, origin point) candidates within the radius win over `curve` (edge, sketch curve, origin axis) candidates within the radius, which win over `area` (face, sketch region, origin plane, datum plane) candidates at distance 0. Within a class the smallest distance wins; at equal distance, items of the active sketch and model elements win over origin items, then the smallest pick id wins. The radius is 4 CSS pixels times the UI scale. In the ID pass, edges and sketch curves are drawn 7 CSS pixels wide, vertices as 9-pixel squares and sketch points as 11-pixel squares (times the UI scale), all depth-tested; faces use a polygon offset so coplanar edges win.

Hover:

- On `pointermove` with no button pressed and no camera gesture, at most once per animation frame, the controller calls `backend.pick` and `choosePick` with the accepted kinds, sets the hover target, and updates highlights. The viewport element mirrors it in `data-hover-kind` (`none` or a `SelectionItemKind`) and `data-hover-key`.
- Mesh hover keys are `mesh:{meshId}:{face|edge|vertex}:{index}` and need no core call. The cursor is `pointer` over a pickable target.
- Hovered faces are tinted with `highlight` (2.16.4) at 20 percent, hovered edges drawn 2.5 CSS pixels wide (times the UI scale, as every size of this section) in `highlight` at 70 percent, hovered vertices as 7-pixel `highlight` dots, hovered sketch items in `highlight`.

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

Selected faces are tinted with `highlight` at 35 percent; selected edges drawn 2.5 CSS pixels wide (times the UI scale) in `highlight`; selected vertices as 7-pixel `highlight` dots. The viewport element mirrors the selection size in `data-selection-count`. A polite live region announces "{label} selected", "{n} items selected" or "Selection cleared", debounced by 500 ms.

Feature highlighting: while the app selection holds features (timeline items, sketch and datum rows) and no feature command is active, the faces those features created are drawn in the selected state. The scene gets them from the core: for every visible body with a model-layer mesh it calls `listElements(documentHandle, body, 'face')` through a `LatestOnly` outside the mutation queue (the command changes nothing), and marks each face whose `ElementDescriptor.feature` is a selected feature id; the face's display id is its position in the returned list (protocol.md 3.4: display ids are positions in `list_elements` order). Lists are cached per body and model-layer mesh id. A list whose body got a new model-layer mesh after the request was sent is discarded and requested again; a new mesh drops that body's cached list. Features whose output has no faces at the display state (sketches, datum planes, suppressed and rolled-back features) highlight nothing.

### 3.7 Feature commands, inspector and live preview

Starting a new feature (`startFeatureCommand(type)`):

1. Cancel an active command; finish sketch mode (remembering the finished sketch); switch to Model. Starts run one at a time: a start (or `editFeature`) asked for while another is still opening its transaction (a double-click, a held key) waits for it, and starting the command that is already active, new and with no change made in it (or editing the feature already being edited, with no change) keeps that command, so a double-click never loses the preselection or sends a second `begin_transaction`.
2. `beginTransaction(documentHandle, manifest.label)`.
3. Generate the feature id (`f-`), set `params = defaultParams(feature, context)` where `lastTargetBody` is the `body` param of the latest active feature in timeline order that has a string `body` param naming a body with model-layer geometry, `bodiesWithGeometry` comes from the model-layer meshes, `context.document` is the recipe and `context.featureStatus` the latest statuses.
4. Preselection: walk the selection fields in manifest order; each takes the app selection items it accepts (in selection order; one item when not `multiple`) that no earlier field took. A selected sketch feature (browser row or timeline item) counts as its region for a `sketch_region` field when its solution has exactly one, and as the points of the 3.3 rule for a `sketch_point` field; this is a preselection rule only, and clicks during the command follow the 2.12 table. The finished-sketch rule of 3.3 applies when the app selection is empty. The app selection is cleared.
5. The active field is the first selection field that is required and empty, skipping a `body` field whose parent object also has a `face` or `edge` field (the first pick fills it, 2.12) and a `body` field the command fills itself (an empty `body` beside op `new` gets the new body, 2.12), so Box, Cylinder and the other primitives open with no active field and focus in their first size; if there is none, the first essential selection field whose control is `multiple` (for example Profile, Edges, Faces to remove), so further clicks keep adding to it; if there is none, no field. Unless the active field is a required empty field, focus goes to the first essential expression field with its text selected; an active field that is not shown (inside the closed More options) gives focus to that expression field too. Whenever the UI moves focus to a field (here, from a status message or after an action, below, and by typing values), it brings the whole field into the inspector body's view, its label included: `focus({ preventScroll: true })` on the input, then `scrollIntoView({ block: 'nearest' })` on its `inspector-field` (`focusFieldInput`, `src/inspector/fields/field-id.ts`). Focus alone scrolls only the input into view, which at 200 percent UI scale left the label under the header.
6. The inspector opens (placement below). If the params are complete, the first preview is applied at once.

Editing an existing feature (`editFeature(id)`): a `sketch` feature enters sketch mode instead. Otherwise: cancel an active command, `beginTransaction(documentHandle, 'Edit {label}')`, `setFocus(documentHandle, id)`, take `params` from the record, mark the command `applied`, and open the inspector titled with the feature label. When the feature's status has `repairs`, the active field is the selection field the first repair's `path` maps to (the mapping rule below), so the next pick replaces the lost reference (4.2); otherwise there is none. Selection fields show `itemsFromValue` and their references are highlighted through `resolveReferences`, each reference paired with its sibling `body` param (features.md 2.5 shapes), on the model layer (which shows the state before the feature). If `setFocus` fails with `focus_inactive`, the transaction is canceled and the core's message is shown as an error notification with the action "Unsuppress" (suppressed feature: unsuppresses it and retries the edit) or "Move marker to the end" (rolled-back feature: `set_rollback` with no id, so nothing is rolled back, then retries). While the command lasts, repair mode (below) shows what the feature lost, so the user sees where to pick again.

Preview cycle: every field change recomputes local field errors (expression states, selection minimums). When the params are complete (every required field set, no field error, no expression pending), the UI submits a preview through a `LatestOnly` over `applyEdits(documentHandle, null, edits)`:

- First application: `add_body` at index `bodies.length` (when the command creates a body, for op `new` with an empty `body` field or for a body-creating `target` set to a new body (2.12): a new body with a `b-` id and `nextBodyName`, and that field is set to it), then the helper sketch's `add_feature` when there is one (placing holes on a face, below), then `add_feature` with the record `{ id, type, ...params }` (no name) at index `features.length` when `rollback` is null, else at the rollback feature's index (the new feature is active; no `set_rollback`); then `setFocus(documentHandle, id)`.
- Later applications: `update_feature` with the full record. Switching op away from `new` removes the body this command created (`remove_body`) and sets `body` to `lastTargetBody`, or clears a body-creating `target`; switching back to `new` adds a fresh body. Clearing a body-creating `target` removes its created body (for Split body this discards the other part); choosing "New body" from its Choose menu creates one again.
- The response's events update the store: `recomputed` gives the feature's status; `body_mesh` events with layer `preview` carry the preview geometry, shown as normal shaded bodies; the model layer shows the state before the feature and stays pickable.
- Incomplete or invalid params after an application hide the preview layer (nothing applied); the next complete state applies again. A preview request (or a parameter edit joining the transaction) that finishes after the params became incomplete or invalid leaves `preview` at `none`.
- `ps:preview` measures from the change to the drawn preview.

Cut direction: in an Extrude command, when Operation changes to Cut or Intersect while the profile's sketch has `plane.kind` `face` and `plane.body` equal to the `body` param, Reverse becomes true; when Operation changes back to Join or New body under the same condition, Reverse becomes false. Once the user has changed Reverse in this command (`changedPaths` holds `reverse`), Operation changes leave it alone. A sketch on a face has its normal pointing out of the body, so a cut sketched on a face goes into the material by default instead of doing nothing.

The inspector shows the focused feature's status messages in a banner (`data-testid="inspector-error"`, `data-severity` the severity of its first message): errors with `role="alert"` on `errorSubtle` with an `errorMark` icon and `errorText` messages, warnings (when there is no error) on `warningSubtle` with the `warningMark` hazard icon and `text` messages, and info notices (when there is neither) on `surface` with the `info` icon in `textMuted` and `text` messages. Each message is followed by its actions, then its cause line and the cause's actions (4.1, 4.2), and Copy details when it has a `detail`; in repair mode the current repair's candidates replace the actions that store them (below). A message whose `field` maps to a shown field is also shown as that field's `field-error`, except an info notice. The banner's actions run as everywhere (4.2, `runFixAction`), with these rules inside the command:

- An `apply` action is `aria-disabled`, with the tooltip `preview_pending` (4.3), while `preview` is `pending`, because the statuses that carry it are about to be replaced. Otherwise `apply_fix` joins the transaction (engine.md section 3.22), with `preview` `pending` until its events arrive, as for a preview; then the command takes the feature's record from the new recipe as its `params` (the helper sketch and the active field stay): one command history step named after the field the message names (Command history, below). Edits the fix made to other features, bodies or parameters join the transaction like the parameter edits of "Other edits during a command" (below): they are not history steps, OK records them and Cancel reverts them.
- A `pick` action of the command's own feature makes the field its `path` maps to the active field (the repair field, below). A `pick` or `show` naming another feature first ends the command as an implicit cancel (below), with Restore.
- After an action, focus goes to the input of the first field, in display order, whose value the action changed (for a toggle such as Reverse, the field it is attached to, so the next Enter confirms instead of toggling it back), else to the field the message's `field` maps to: the banner goes once the fix clears its message, and focus has to stay in the inspector for the Enter ladder (3.5).

The UI adds one action of its own, "Reverse and use {text}" (4.2), which is exact; every other fix is the core's, so a fix the core did not verify (a Reverse that misses too) is never offered. Mapping rule: a hint path `a.b[1]` is the pointer `/a/b/1`; the field is the hint whose pointer equals `field`, else the hint whose pointer is the longest whole-segment prefix of it (so `/edges/1` maps to `edges`). The same rule finds the field for `Repair.path`.

Confirming (Enter ladder, or the OK button): allowed only when complete, and once: a second Enter or click while the first confirm runs does nothing. `flush()` of the preview `LatestOnly` is awaited first, so the latest params are applied. Then `commitTransaction(documentHandle)`, the inspector closes, the command clears and focus returns to where it was before the command. A sketch referenced by the committed params through a `sketch` key is hidden for the session (3.10). If the committed feature's status is `error`, the failure notification of 4.2 appears.

Canceling (Esc ladder, the Cancel button or the close button): `cancelTransaction(documentHandle)`, the inspector closes, nothing is recorded.

Other edits during a command: while a feature command's transaction is open,

- Parameter changes from the Parameters panel (add, expression, comment, delete) are sent through `applyEdits` with no label, so they join the transaction (a rename already does, protocol.md command 31), and the preview follows at once: while such an edit is in flight `data-preview` is `pending`, and it returns to `shown` (or `failed`) when the edit's events arrive. OK records them with the feature as one undo entry and Cancel reverts them. They are not command history entries: `Mod+Z` with an empty history cancels the command and reverts them with it, and Restore (below) does not bring them back. The Parameters panel shows under its title, in `textMuted`: "Changes are part of {title} until you press OK." (`data-testid="parameters-transaction"`).
- Session view state stays available: sketch, datum and origin eyes, body locks, the section view, the camera, the selection filter, the grid and every panel.
- Every other document edit is disabled with `finish_command_first`: Save and Save as, body visibility, color, material, rename and delete, timeline rename, suppress, unsuppress, delete, reorder and the rollback marker, units and printer, and the `apply` actions of messages outside the command's banner (Problems panel, notifications). Their controls are `aria-disabled` with the reason as tooltip; the timeline drag and the marker do not start. The actions themselves refuse these edits during a command and show the reason as an info notification, as a disabled command does when clicked (2.10), so no path sends a labeled edit into the open transaction.
- New, Open, Open recent, a dropped file and the recovery Open cancel the command first, explicitly (no `command_canceled`), before the replaced document is stashed and closed, so the stash never holds the command's preview.

Command history: each change the user makes in a command (a pick added or removed, a field cleared, a choice, switch or toggle changed, the first text change of an expression field after it gains focus, a fix action of the banner, a "New body" choice) pushes a `CommandStep` (the params, helper sketch and active field before it, with the changed field's label) onto `history` and clears `future`; its path joins `changedPaths`. Preselection and starting values are not changes, and the rules a change triggers (the sibling body of 2.12, the cut direction above) belong to that change. With focus outside a text field, `Mod+Z` pops `history`, restores that state (including the helper sketch of placing holes on a face) and previews again; `Mod+Shift+Z` pops `future` likewise. With an empty `history`, `Mod+Z` cancels the command and calls no core `undo`. In a text field, `Mod+Z` stays the field's own text undo (2.10 dispatch rule 2). So a reflex `Mod+Z` after a wrong pick undoes that pick, not every pick.

Implicit cancel: when a command whose `history` is not empty ends without OK or Cancel because the user did something else (another feature command starts, another feature is edited, a sketch is entered, or the context changes), it is canceled as before and the info notification `command_canceled` offers "Restore". Restore cancels whatever command is then active, explicitly (that cancel raises no notification of its own), and reopens this one (`startFeatureCommand` without preselection, or `editFeature` of the same feature) with the saved params, helper sketch and active field. A body the saved command had created is made again: a body-creating `target` gets a fresh new body, and a `body` filled by op `new` is emptied so the first application creates one; the reopened command starts with an empty history. New, Open, Open recent, the recovery Open and a core restart cancel without this notification, because Restore would reopen the command against another document. A command whose `history` is empty ends silently, as before. Committing instead of canceling is deliberately not done: a newcomer who clicked Extrude and then Revolve would get a feature they did not want and a hidden sketch.

Inspector panel (`role="dialog"`, `aria-modal="false"`, `aria-labelledby` its title, `data-testid="inspector"`, `data-mode="command"`, `data-preview` mirroring `ActiveCommand.preview`: `none` before anything is applied or while the params are incomplete, `pending` while a preview request is in flight, `shown` when the preview layer for the latest params is drawn, `failed` when the focused feature's status is `error`), `w-80`, at most the free area's height minus 2rem (`max-h-[calc(100%-2rem)]` in the free-area layer, 3.2, the inspector's one arbitrary value: its height depends on the free area, and it is absolutely positioned at a runtime translate), a card (3.2) that never scrolls as a whole (`flex flex-col overflow-hidden`; its body under the header scrolls, `min-h-0 flex-1 overflow-y-auto`, as an island's does, and the footer below the body stays in view, `shrink-0` with no rule of its own, so Cancel and OK never scroll away; the body's bottom rule shows only while fields continue under the footer, 3.2): `surface` with a `forcedEdge` border, `rounded-xl` and `shadow-lg` with `elevation`:

- Header: feature icon, title (manifest label for a new feature, feature label when editing; the title has the command's description, 2.10, as its tooltip), close button "Cancel". The header is the drag handle.
- Status banner as above.
- Essential fields, then a disclosure button (`data-testid="inspector-more"`, `aria-expanded`) when there are more fields: "Fewer options" while open; while closed "More options", followed by " · " and the `changedMore` labels joined with ", " when there are any ("More options · Tip angle"), so a value changed out of sight never surprises anyone. Its tooltip lists every field in More. Its state is remembered per feature type in `settings.moreExpanded`; `forceMore` opens it, so More opens by itself only when a field in it needs attention (an error or a required empty value).
- Footer: "Cancel" and "OK" (`data-testid="inspector-ok"`, primary: `accent` fill); Cancel's tooltip shows "Cancel" with "Esc", OK's "OK" with "Enter" when complete. OK is `aria-disabled` while incomplete, with the tooltip "Fill in the highlighted fields first."; required empty selection fields get a dashed `accent` border and invalid fields an `errorMark` border.
- Each field is a container with `data-testid="inspector-field"` and `data-path`.

Field controls:

- Labels: a field whose `help` (2.12) is not null shows it in a tooltip when the pointer rests on its label (the label has `cursor-help`, the only cue, so the layout gains nothing), and the help is also a visually hidden description (`sr-only`) referenced by `aria-describedby` from the control. Labels without help have no tooltip.
- Axis letters: a field with an `axis` (2.12: Move X to Move Z, Position X to Position Z) shows its letter inside the input at its start (`AxisLetter`: `data-testid="axis-letter"`, `data-axis` the letter, `data-world`, `aria-hidden="true"`, `pointer-events-none absolute inset-y-0 left-0 flex w-6 items-center justify-center text-xs font-semibold`, and the input gets `pl-6`), in the axis role's foreground (`axisX`, `axisY` or `axisZ`, 2.16.3) when `world` is true and in `textMuted` when it is false, since the letter then names an axis of the primitive's plane, not a world axis. The label keeps its text (Move X), so the field's accessible name and the letter's meaning never depend on color, and the letter matches the X, Y and Z of the view cube, the origin axes and the readouts of 3.8.
- Expression: `ExpressionInput` (2.13) labeled with the hint label, its text in the interface face with `tabular-nums` whatever it holds (2.16.7). To its right, `data-testid="expression-result"` (`tabular-nums`, `textMuted`) shows the unit symbol when `isPlainNumber(text)` holds and the text has no unit word (`mm`, `°`), otherwise `= {formatQuantity(value)}` (`= 10 mm`), or nothing while empty or pending; with nothing to show it is hidden (`empty:hidden`), so it takes no gap and the input ends where the row's other controls do. Below it, `data-testid="field-error"` (`aria-live="polite"`, referenced by `aria-describedby`) shows the message of an invalid state. ArrowUp and ArrowDown step plain numbers (`stepExpression`; Shift coarse, Alt fine) and do nothing otherwise. In the command and selection inspectors, when the field is invalid with a non-null `range` whose text is an identifier that names no parameter and does not start with `printer.`, `field-error` is followed by a button "Add parameter {name}". It opens the Parameters panel with a draft row named {name} whose expression is `formatQuantity` of the field's last valid value (empty when it never had one), with the expression input focused. Committing the draft adds the parameter (joining the open transaction during a command, above) and returns focus to the field, which re-evaluates at the new revision; Esc discards the draft and returns focus. So a parameter is created from the field that needs it.
- Selection: a box (`data-testid="selection-field"`, `role="button"`, `data-active`, `data-count`) showing the placeholder `Select {noun}` (`Select edges or a face` for a `multiple` `edge` field, `Select a plane`, `Select regions`, `Select a body`, `Select bodies`, `Select a sketch`, `Click a flat face or pick sketch points` for `sketch_point`, `Select faces`, `Select an axis`), one item's label, or `{n} {plural noun}` (`3 edges`). A field set to the command's created body reads "{name} (new)". Clicking it makes it the active field (`accent` border instead of `borderControl`); a clear button "Clear {label}" empties it. For `body`, `sketch`, `plane` and `axis` fields and for `sketch_region` fields, a "Choose" menu button lists the enumerable candidates (bodies with geometry; sketches; origin planes and datum planes; origin axes; regions of the current profile's sketch, or of every visible sketch when empty, as "Region {n} ({area})") so these fields work without a pointer. For a body-creating `target` (2.12) the Choose menu lists "New body" first, then the bodies without model-layer geometry (a target must be empty). Hovering the box highlights its items in the viewport: while the pointer is over it, `ActiveCommand.hoveredField` names the field, and the scene draws the field's items in the hover state (its faces, edges and vertices through the references the controller resolved, its bodies, regions, points and planes by key).
- Choice: a segmented control (`role="radiogroup"`) for up to 4 options, else a native `<select>`, with the hint's option labels. The segmented control spans the field's row, its segments sharing the width (`flex-1`, `px-2`, so a bold checked label never pushes it wider). When its segments do not fit the row (the field's row is narrower than their content, as for Extrude's Extent: Distance, Symmetric, Two sides, Through all), the field shows the native `<select>` instead, so no option is ever clipped (`appearance-none pr-8` with its own `chevron-down` icon, `size-4` in `textMuted`, since the browser's arrow keeps its size at every UI scale): the row is measured before the browser paints and whenever the control changes size, and the segmented control is tried again once the fonts have loaded.
- Switch: `role="switch"`. Toggle: an icon button (`arrow-up-down`, `size-8`, accessible name the field label, `aria-pressed`, the field's help as its tooltip) at the end of the row of the field it attaches to (`attachedTo`, 2.12), so Reverse sits beside the distance it reverses; it keeps its own inline `inspector-field` container with its `data-path`. Text and number: text inputs; number fields set their digits in tabular figures (`tabular-nums`, 2.16.7) and reject non-numeric text with "{label} must be a number."
- Unsupported: read-only compact JSON with the help "Edit this value in the .ptsm file."

Limits under fields: while a fillet, chamfer or shell command's preview status has a `fillet_too_large`, `chamfer_too_large` or `shell_too_thick` message with a `suggestion` (the largest value the core's search found to work, features.md section 4.3), the field the message's `field` maps to shows `field_limit` ("Largest that works: {suggestion}", 4.3) under its `field-error` (`data-testid="field-limit"`, `warningText`, referenced by the input's `aria-describedby` after `field-error`), and its border is the `warningMark` border instead of `errorMark`; the message's actions stay in the banner. In a document whose units are not mm, a field whose text is a number without a unit (`isPlainNumber` without a unit word) shows no such line, because there the suggestion is that number in mm, which keeps the size the user meant (features.md section 4.4), not the largest value that works.

Selection capture (`capturePick(item, pointer)`): when the active field accepts the item (`acceptsTarget`; geometry checks such as a flat face use the item's `ElementDescriptor`), the item is toggled in the field: added (replacing the single value when not `multiple`) or removed when already present. While the edited feature's lost reference (a `Repair` whose `path` points into this field's list, such as `/edges/0` or `/profile/regions/1`) is still in the field, a pick replaces it instead, at its place in the list (a face picked into Edges puts its edges there; an item the list already holds just removes the lost one), so the warning and its repair go once it is picked again; undoing that pick brings the lost reference back for the next pick. Later picks toggle as usual. A refused item shows a hint under the field (`pick_plane` for `plane`, `pick_axis` for `axis`, `pick_points` for `sketch_point`, `pick_kind` otherwise; 4.3) for 3 s. When a non-multiple field receives its item, or a `multiple` field receives its first item, the active field advances to the next required empty selection field, if any. Clicking a field's box makes it the active field again. Focusing an expression field does not change the active field, which keeps its `accent` border, so clicks on the model still go to it while the user types. The sibling `body` rule of 2.12 applies.

Repair mode: shown for one feature at a time while a feature whose status has `repairs` is being edited, or, with no feature command active, while focus is inside its Problems panel row (3.9, the feature `AppState.repair` names), and opened by "Show what '{label}' lost" (`showLost`: it edits the feature, or re-attaches a sketch whose face is lost, 3.16.1, and frames what was lost: `fitBounds` of the ghost's points, else of the `centroid`, together with the candidates' markers, grown about their center to at least a third of the size of the body's bounding box on each axis, margin 0.1; for a region repair, of the candidates' numbers, grown to a third of the size of the sketch's drawing). Its *current repair* is the first repair whose `path` maps to the active field (the mapping rule above), else the first repair; activating another field that holds a repair makes that one current. While it is shown:

- Each element repair of the feature draws its `ghost` on the overlay (3.12) as a dashed `errorMark` outline where the element was in its last build (`data-testid="repair-ghost"`, an SVG `path` with the static attribute `stroke-dasharray="4 3"`): the segments for `lines`, the outline of the triangles (the triangle edges that belong to one triangle only) for `triangles`, and a ring of 8 CSS pixels diameter for `points`. A repair without a ghost gets the dashed ring of 16 CSS pixels diameter at its `centroid` (`data-testid="repair-marker"`, an SVG `circle`), and one without either draws nothing. Each has the SVG `title` "{Description} (missing)" (`repair_missing`, 4.3) when the repair's `resolution` is `missing`, and "{Description} (check it)" (`repair_check`) when it is `ambiguous`: an ambiguous loss, or a guessed or widened reference or a split, resized or matched-again region, which are still used (features.md section 4.2); `{Description}` is the `description` with its first letter capitalized. The ghost lies in model coordinates, is drawn without depth test, and shows through the part, where the element often is now.
- Each candidate of the current repair gets a numbered marker (`data-testid="repair-candidate"`, `data-number`, a `rounded-sm` chip with the number in `tabular-nums`, positioned per 3.12) at the centroid of its `element`: the `centroid` of the `listElements` descriptor of that kind at display id `element.index` of the body's model-layer mesh `element.mesh_id`, cached as in 3.6. A candidate without `element`, or whose mesh is no longer the body's model-layer mesh, gets no marker.
- The banner lists, under the current repair's message when the banner shows it (a warning that carries the repair is not shown under an error), its candidates as buttons (`data-testid="repair-candidate-button"`) reading "{n}. {Description}", with `relation` on a second line in `textMuted`, in the order received. No percentage or score is shown anywhere. Each runs the message's action that stores the candidate (`candidateAction`, below), which is then not shown again among the message's actions; a candidate without one is listed, `aria-disabled`, so the list keeps its numbers.
- Hovering a candidate's button or marker draws its `element` hovered (3.6), and clicking either runs its action. In repair mode opened from a Problems row, that action is applied at rest (4.2).
- The repair field is the active selection field, so any accepted pick on the model replaces the lost reference (Selection capture, above). "Remove from {label}" and "Pick {noun} again" stay among the message's actions.
- A region repair draws its `ghost` (frame `sketch`) on the plane of its `sketch`, with that sketch's latest solved placement, in the same dashed style, and each candidate set fills its regions in `highlight` at 20 percent with its number at the centroid of its first region; this holds in Model context and while that sketch is edited.
- Esc leaves repair mode with the command (Esc ladder, 3.5). Repair mode also ends when the feature has no repair left, and when focus leaves a Problems row that opened it.

```ts
// src/repair/repair.ts
export type RepairCandidateView = {
  readonly number: number
  readonly description: string
  /** What it keeps and how far it is; empty for a region candidate, which has no second line. */
  readonly relation: string
  /** The candidate's element in a model-layer mesh; null for a region candidate. */
  readonly element: MeshElement | null
  /** The message action that stores the candidate, or null. */
  readonly action: FixAction | null
}
/** The current repair of a feature and its numbered candidates (3.7), or null when its status has no repair. */
export function repairView(status: FeatureStatus, activePath: string | null): { readonly repair: Repair; readonly candidates: readonly RepairCandidateView[] } | null
/** The `apply` action of `messages` labeled `Use {description}`, `Use only {description}`, `Attach to {description}` or `Attach '{sketch}' to {description}` (engine.md section 3.22), or null. */
export function candidateAction(messages: readonly Diagnostic[], description: string): FixAction | null
/** The segments of a ghost's points as world points, two per segment: `lines` as given, the boundary of `triangles`; empty for `points`. */
export function ghostSegments(kind: GhostKind, positions: Float32Array): Float32Array
```

`candidateAction` takes the first matching action of the message that carries the repair; the label is the only link between a candidate and its action on the wire (open issue 18).

Faces for edges: while the active field accepts `edge` and is `multiple` (Fillet, Chamfer), faces are accepted too: clicking a face adds every edge named in its `ElementDescriptor.adjacent` that the field does not hold, in `adjacent` order, or removes them all when it holds every one. Each edge's display id is its position in the `listElements(documentHandle, body, 'edge')` descriptors (cached per body and model-layer mesh id as in 3.6), and its reference is the full one `pickReference` returns for that display id in the same mesh, as a click on the edge would store it (2.12), so the UI still never builds a reference. The edges are added when every `pickReference` has answered; a `stale_pick` (a new mesh meanwhile) adds nothing. Hovering a face in this state highlights its edges instead of the face: the viewport controller lists the body's faces and edges once per model-layer mesh (display ids are list positions, protocol.md 3.4) and hands the edge display ids to the scene (`Scene.setHoverEdges`). The sibling body rule (2.12) applies.

Placing holes on a face: while the active field accepts `sketch_point`, a click on a flat face (`surface` `plane`) of a visible, unlocked body places a hole center, so Hole works from a face as well as from sketch points, with a real sketch behind it:

- The center is where the click's `PointerRay` meets the face's plane (`ElementDescriptor.placement`), in that placement's sketch coordinates (sketch.md); when the face's `centroid` is within 8 times `millimetersPerPixel` of that point, the center is the centroid.
- The first such click gives the command a helper sketch (`ActiveCommand.helper`): a `sketch` record with a new `f-` id, `plane` `{"kind": "face", "body": <the face's body>, "face": <its Reference>, "fallback": <its fallback>, "fallback_frame": "body"}` exactly as `createSketchOn` builds it (3.16.1), one point entity `{"id": <new e- id>, "type": "point", "position": [x, y]}` and no constraints. Points becomes that entity's `PointRef`, the sibling `sketch` field the helper's id and the `body` field the face's body; points of another sketch that Points held are replaced (the 2.12 rule for points of another sketch).
- Further clicks on the same face add a point entity to the helper and its `PointRef` to Points. The helper's points are drawn from the command state like sketch points as soon as they exist, before any preview, on the face's world placement the helper keeps (`HelperSketch.placement`), and are pickable, so clicking one removes it (`capturePick`'s toggle) along with its point entity; when Points holds none of the helper's points anymore, the helper is dropped and a Sketch field naming it is cleared. A click on another face is refused with `hole_one_face`, and a face that is not flat with `pick_points`. Without a `PointerRay` (a pick that did not come from the viewport) the center is the centroid. Centers are kept to 6 decimals of a millimeter.
- Every preview application carries the helper: the first one sends `add_feature` of the helper at the index the marker rule gives the hole, followed by the hole's `add_feature` right after it (when editing, the helper goes at the hole's index, before it); later ones send `update_feature` of the helper. When Points no longer holds a point of the helper, the next application removes it (`remove_feature`) and `helper` becomes null.
- Confirming commits the helper and the hole in the one transaction (one undo entry, the command's label); Cancel discards both. The committed helper is hidden like any sketch the committed params reference (Confirming, above); opening it later (double-click its browser row) lets the user dimension the centers exactly.

Typing values: the command's value field is the expression field the user changed last in this command (the last expression path in `changedPaths`), else the first essential expression field. While a feature command is active and focus is on the viewport or on a selection field box, a digit, `.`, `-` or `(` pressed without Mod, Ctrl or Alt focuses the value field, replaces its text with that character and leaves the caret after it; Tab on the focused viewport focuses the value field with its text selected. The Enter ladder (3.5) then confirms, so pick, type `2`, Enter makes a 2 mm fillet. Letters keep their shortcuts. The typed character is one history step of the value field (Command history, below). `layout/use-typed-values.ts` takes these keys in the capture phase on `window`, before the viewport's own keys, so during a command `-` starts a negative value instead of zooming out (3.13).

Placement: when the inspector opens, it is placed with `placeInspector` in the free-area layer's coordinates, where `anchor` is the screen rectangle of the preselected items (or null) moved by the free area's offset in the canvas, and `bounds` the free area with three sides moved in where the panel still fits: its left side to the right edge of `viewport-controls` (3.14) when that leaves at least the panel's width plus two gaps, so an inspector that opens in the corner never covers the controls, its right side to the left edge of the view cube's reach (its box widened to its Home view button, 3.14) when that leaves at least the panel's width plus two gaps, so a card placed right of its items never covers the cube, and its top side to the bottom of the top-center slot's column (`top-center-slot`, 3.2) while the slot shows a card or chip (the empty state, the sketch header and hint, the print hint) and that leaves at least the panel's height plus two gaps, so it never covers them. Otherwise that side stays at the free area's edge (the controls' commands keep their shortcuts, 3.14). It then stays put while open, inside the free-area layer: a kept dragged position is clamped so the whole card lies in the free area when an inspector opens, and a free area that shrinks under the card (a docked island opens, the window narrows) or a card that grows moves it back inside (a `ResizeObserver` of the layer and the card), so it never ends up under an island. Dragging the header moves it within the free area, and the dragged position (`panels.inspectorPosition`) is used for every inspector until reload. The position is applied with `positionElement` (3.12), never through a `style` prop.

### 3.8 Selection inspector

With no feature command active and a non-empty app selection (Model and Print contexts), or a non-empty sketch selection with no tool active (sketch mode), the inspector shows the selection (`data-mode="selection"`, accessible name "Selection"). It uses the same panel, placement and dragging rules, and closes when the selection empties.

| Selection | Title (`data-testid="selection-title"`) | Rows and actions |
| --- | --- | --- |
| One body | Body name | Size (`data-testid="body-size"`, the extents of the model-layer bounding box as a coordinate readout, below, or "Empty"), Center (`data-testid="body-center"`, the box center as a coordinate readout), Color (swatch button), Material (combobox with suggestions PLA, PETG, ABS, ASA, TPU, Resin; Enter commits "Change material"; empty clears), Visible (switch). Suggested commands: Move body, Mirror, Split body |
| One feature, sketch or datum | Feature label | Type (manifest label), status messages with their actions, cause lines and notes (4.2), buttons "Show what '{label}' lost" when its status has repairs, "Edit", and "Suppress" or "Unsuppress". For a sketch, suggested commands: Extrude and Revolve when its solution has regions, Hole when it has non-construction points |
| Faces, edges and vertices | "1 face", "3 edges", "2 vertices", or "{n} items" when kinds mix | Body name(s), each a button "Select {name}" that replaces the selection with that body; for one face, edge or vertex whose `element.feature` names a feature in the document, a row "Made by {label}" with a button "Edit {label}" (`editFeature`; for a face its tooltip adds "Or double-click the face."), and hovering the row gives that feature's timeline item the `hover` role's `active` background; for one face its surface ("Flat face", "Cylindrical face", "Conical face", "Spherical face", "Toroidal face", "Freeform face", "Face") and area (`formatNumber` in the document unit followed by `{unit}²`); for one edge its curve ("Straight edge", "Circular edge", ...) and length; for one vertex its position (`data-testid="vertex-position"`, a coordinate readout). Suggested commands: edges: Fillet, Chamfer; one flat face: Sketch, Hole, Shell; other faces: Shell |
| Regions | "1 region", "{n} regions" | Suggested commands: Extrude, Revolve |
| Sketch points (Model and Print contexts) | "1 point", "{n} points" | Suggested command: Hole |
| Two or more bodies | "{n} bodies" | Suggested command: Boolean |
| One origin plane or datum plane | "Top plane (XY)", "Front plane (XZ)", "Right plane (YZ)" or the datum label | Suggested commands: Sketch, Datum plane |
| One origin axis or the origin point | "X axis", "Y axis", "Z axis" or "Origin" | none |
| One body and one origin or datum plane | "2 items" | Suggested commands: Mirror, Split body |
| Anything else mixed | "{n} items" | none |
| Sketch entities and points | "1 line", "2 lines", "1 circle", "1 arc", "1 point", "{n} items" | Constraint suggestions (`applicableConstraints`, each a button with glyph, title and shortcut), Construction (switch, for entities), Delete. For one line, circle or arc, a size field (below) |
| One sketch constraint | Constraint title ("Horizontal", "Distance") | For dimensions: an expression field with the stored expression (Enter commits "Change dimension") and a Reference switch; Delete |

Suggested commands (`suggestedCommands` in `src/inspector/suggestions.ts`, the rows above) are buttons with the command's icon, title and formatted shortcut (the shortcut `aria-hidden` and given through `aria-keyshortcuts` with `ariaKeyShortcut`, 2.10, as for the constraint suggestions). They run the command with the selection preselected (3.7 step 4). A suggestion whose command is disabled (2.10) is left out. So commands that are in the palette only (Datum plane, Move body, Split body) are offered where they apply, with no toolbar growth. A timeline item of a datum plane counts as a datum plane.

```ts
/** The `feature.<type>` commands the rows above suggest for the selection, in order; a type the catalog lacks is left out. None in Sketch context. */
export function suggestedCommands(state: AppState, items: readonly SelectionItem[]): readonly string[]
```

The "Made by {label}" row (`data-testid="made-by"`) sets the app hover to `{key: 'feature:{id}', kind: 'feature'}` while the pointer is over it, which the timeline item of that feature shows with the `hover` role's `active` background; the "Select {name}" buttons show that text.

Size field: for one line, circle or arc in the sketch selection, a field (`data-testid="entity-size"`, `ExpressionInput` with unit length) labeled "Length", "Diameter" or "Radius". Its text is the expression of the driving dimension already on the entity (a `distance` on the line or on its two end points; a `diameter` or `radius` on the circle or arc) when there is one, else `formatNumber` of the solved size. Enter with a changed valid text commits through `commitSketchChange`: with a driving dimension, its replacement labeled "Change dimension"; otherwise a new driving dimension (`dimensionFor`: `distance` for a line, `diameter` for a circle, `radius` for an arc) at `defaultLabelPosition`, labeled "Dimension", which then shows on the canvas. When the new dimension would over-constrain the sketch, the field shows `dimension_over_constrained` with "Add reference" (adds it with `driving: false`) and "Cancel" (restores the text) (3.16.7); a replaced dimension the sketch cannot take shows the probe's message (3.16.7), and a refused change its message, as the field's error. Enter with the text unchanged commits nothing. So a shape's size can be typed after selecting it, with expressions and parameters as everywhere else.

```ts
// src/inspector/entity-size.ts
export type EntitySize = {
  readonly entity: string
  readonly label: 'Length' | 'Diameter' | 'Radius'
  readonly type: 'distance' | 'diameter' | 'radius'
  /** The driving dimension's expression, else `formatNumber` of the solved size. */
  readonly text: string
  /** The driving dimension already on the entity, which a change replaces; null without one. */
  readonly dimension: SketchConstraint | null
  /** A point of the entity, which orders a new dimension's operands. */
  readonly at: Vector2
}
/** The size field of the sketch selection: one line, circle or arc; null otherwise. A stored radius on a circle keeps the field a Radius. */
export function entitySize(items: readonly SelectionItem[], params: SketchParams, solution: SketchSolution, units: Units): EntitySize | null
/** The dimension a typed size becomes: the driving dimension with the new expression, or a new one (`dimensionFor`), only measuring with `reference`. */
export function entitySizeDimension(size: EntitySize, item: SelectionItem, expression: string, solution: SketchSolution, ids: IdSource, options?: { reference?: boolean }): SketchConstraint
```

Measurements: selecting something measures it, with no tool and no mode. Rows (`selectionMeasurements` in `src/inspector/measurements.ts`; `data-testid="measure-row"`, `data-measure`) show a label and a value with `tabular-nums` (selectable text, `formatQuantity`):

| Selection | Rows (`data-measure`) |
| --- | --- |
| One circular edge or cylindrical face | Diameter (`diameter`): 2 × the descriptor's `radius` |
| One spherical face | Diameter (`diameter`): 2 × the descriptor's `radius` |
| One body with geometry (Size is not "Empty") | Volume (`volume`): `measure` with `{kind: 'body', body}` through a `LatestOnly` keyed by the body's model-layer mesh id; the volume divided by the cube of millimeters per document unit, rounded as a length (2.13), followed by `{unit}³`; empty while pending |

A body's rows follow its Center; an element's follow its surface or curve rows. Each model-layer mesh is measured once per app; a failed `measure` leaves the value empty.

Coordinate readouts (Size, Center and a vertex's position) give each value after its axis letter: `X 40  Y 20  Z 10 mm` (`AxisValues`, `src/components/AxisLetter.tsx`). A readout row has two lines: its label (`text-xs`, `textMuted`), then the values, `flex w-full flex-wrap items-baseline gap-x-2 gap-y-1`, which hold three cells `flex flex-1 basis-0 items-baseline gap-1`, each `{letter} {formatNumber(value)}`, and then the unit in a `shrink-0` span. Each letter is an `AxisLetter` without `aria-hidden` (`data-testid="axis-letter"`, `data-axis`, `font-semibold`, the axis role's foreground), each number has `tabular-nums` (2.16.7), and a space text node separates each letter from its number, the cells from each other and the last cell from the unit (flex containers do not render it), so a screen reader and a test read "X 40 Y 20 Z 10 mm". The cells grow from a zero basis, so every readout spans the inspector's content width in equal thirds, and Size and Center line up letter over letter by construction; a cell never shrinks below its content (a third holds about 7 characters of `text-sm` values, such as `-123.45`), so a longer value wraps the row instead of overflowing or being cut. The values element (`body-size`, `body-center`, `vertex-position`) keeps the plain text in `data-value` (`formatSize` or `formatPoint`, 2.13), which tests compare (6.5); a body without geometry has the Size "Empty" (`data-value="Empty"`, no letters) and no Center row. Distances and deltas between two items, later (1.2), use the same cells.

```ts
// src/inspector/measurements.ts
export type MeasureRow = { readonly measure: 'diameter' | 'volume'; readonly label: string; readonly value: string }
/** The rows of the table above; a volume not measured yet has an empty value. Reads the state and the app's volume meter only. */
export function selectionMeasurements(app: App, items: readonly SelectionItem[]): readonly MeasureRow[]
/** Asks the core for what the rows still need (the volume of one body's mesh); resolves when it answered. */
export function measureSelection(app: App, items: readonly SelectionItem[]): Promise<void>
/** A volume in mm³ in the document unit cubed: "8000 mm³", "0.4882 in³". */
export function formatVolume(volume: number, units: Units): string
```

Distances and angles between two selected items need the core's two-target `measure`, which is later (1.2).

### 3.9 Timeline

Layout, left to right: collapse button ("Collapse timeline" / "Expand timeline"); the items in timeline order with the rollback marker in the gap before the rollback feature (the first rolled-back feature; at the end when nothing is rolled back); at the right end, the problems button (`data-testid="timeline-problems"`, text "1 problem" or "{n} problems", n from `selectProblemCount` (Problems panel, below), after the icon of the worst problem: `circle-x` in `errorMark` when any feature or diagnostic is an error, else the `warningMark` hazard sign, as status icons are everywhere (3.24); hidden when there are none). The item strip scrolls horizontally; a newly added item and the marker scroll into view. With no features, the item strip shows `text-xs` text in `textMuted` (`data-testid="timeline-empty"`): "Each step you add appears here. Double-click a step to change it later."

Items (`data-testid="timeline-item"`, `data-feature-id`, `data-status` from the latest `recomputed` status (`ok`, `warning`, `error`, `suppressed`, `rolled_back`), or `pending` before the first status for a new feature, `data-suppressed`, `data-rolled-back`):

- A `size-10` button with the feature icon (`size-5`). Its accessible name is the feature label; `aria-describedby` points at a hidden text with the status ("Error: {first message}", "Warning: {first message}", "Suppressed", "Rolled back").
- A status badge at the bottom-right: the `triangle-alert` hazard sign (`warningMark`) for warnings, `circle-x` in `errorMark` for errors, and for a feature whose status is `ok` with info notices a small `info` mark in `textMuted` (`data-notes` on the item is the number of its info messages, 0 without). The tooltip shows the label on its first line and every message, in order, on its second line in `textMuted` (the description line of 3.24).
- Suppressed items draw their icon at `opacity-40` with a diagonal strike (an absolutely positioned bar `inset-x-1 top-1/2 h-0.5 -rotate-45 bg-current` in `textMuted`). Items after the marker draw at `opacity-40`. While a feature is being edited, it gets an `accent` outline (`outline-2`) and items after it draw at `opacity-40`; the sketch being edited gets an `accent` outline.
- Selected items have the `selection` background and `aria-pressed="true"` (others `false`), so assistive technology hears which are selected.

Interactions:

- Click selects the feature (Shift or Mod toggles), which highlights the faces it created (3.6). Double-click or Enter edits it (`editFeature`; a held Enter's repeated presses do nothing). F2 renames inline: a text field over the item; Enter commits `update_feature` with the new `name` (label "Rename"); an empty name removes `name` (the default label returns); Esc cancels.
- Context menu (right-click, Shift+F10, ContextMenu key; `role="menu"`, `data-testid="context-menu"`): Edit, Rename, Suppress or Unsuppress, Roll back to here, Delete, and, when other features use this one, `used_by` ("Used by {labels}", 4.3) right after Suppress or Unsuppress. "Roll back to here" moves the marker to the gap after the item, so the item stays active (`setRollback` with the item's 1-based position). The users are `usersOf` (below): at most three labels in timeline order, then " and {n} more"; choosing the item selects them all, which highlights what they made (3.6). Opening the menu runs `refreshUses`; the item appears once the uses of the current revision are known.
- Suppress: `update_feature` with `suppressed: true` (label "Suppress {label}"); unsuppress likewise ("Unsuppress {label}"). Several selected features change in one edit.
- Delete (Delete key or menu): one `apply_edits` labeled "Delete {label}" or "Delete {n} features" with `remove_feature` edits; when the rollback feature is among them, a `set_rollback` to the nearest later feature that is not deleted (no id when none) comes first. An info notification "Deleted {label}." offers "Undo", unless the delete raises a loss or failure notification (4.2), which names what broke and offers "Undo" itself. A suppress likewise raises those notifications for what it breaks.
- Reorder: pressing an item and moving more than 4 CSS pixels horizontally starts a drag: a ghost follows the pointer and an insertion bar (`w-0.5`, `accent` background) marks the nearest gap; both are moved with `positionElement` (3.12). Dropping at gap g (0 to N) moves the feature to index g when g is at most its current index, else g minus 1, so the gaps on either side of the feature are its own position, where a drop does nothing. The edit is `move_feature` labeled "Reorder". Esc cancels the drag. The core does not check dependencies when moving (model.md 3.9); a feature that now comes before something it uses fails or loses a reference and triggers the notifications of 4.2, whose "Undo" restores the order.
- Drop warning: a drag runs `refreshUses` when it starts. While the nearest gap would break a use (`dropWarning`, below), the insertion bar has the `warningMark` background instead of `accent`, and a chip (`data-testid="drop-hint"`, `role="status"`, `surface` with a `forcedEdge` border and `shadow-lg` (3.2), `text-xs`, positioned per 3.12 above the bar) shows `drop_breaks_use` (4.3), for example "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?". Dropping there anyway is allowed and undoable, with no confirmation; dropping elsewhere, or before the uses are known, never warns.
- Rollback marker (`data-testid="timeline-marker"`): a `w-2` handle with a `w-0.5` `accent` line, `role="slider"`, `aria-label="Rollback marker"`, `aria-valuemin="0"`, `aria-valuemax` the feature count, `aria-valuenow` the number of active features, `aria-valuetext` "At the start" (p = 0), "After {label}" (the label of feature p) or "At the end" (p = N). Position p (0 to N) means features 1 to p are active; p = N writes `set_rollback` with no id, p < N writes the id of feature p + 1 (the first rolled-back feature).
  - Dragging: `beginTransaction(documentHandle, 'Roll back')`, then on every gap change a `LatestOnly` `apply_edits` with the `set_rollback`, so the model updates live; dropping commits; Esc cancels the transaction. Dropping at the starting gap cancels.
  - Keys: ArrowLeft and ArrowRight move by one gap, Home to position 0, End to the end; each key press applies its own edit labeled "Roll back".
- Collapsed (`h-7`): only the expand button, the text "{n} features" and the problems button remain.
- Keyboard: the strip is a toolbar with one tab stop; Left and Right move focus between items and the marker; Space selects; Mod+Space toggles.

`src/timeline/uses.ts` reads `document.uses` (`list_uses`, engine.md section 3.24), which `refreshUses` keeps at the document's revision: it calls `listUses` outside the mutation queue through a `LatestOnly` when `document.uses` is null or older than `document.revision`, and stores the answer only while its revision is still the document's.

```ts
/** A use a drop would break: `user` would come before `feature`, whose `what` it uses. */
export type DropWarning = { readonly user: string; readonly feature: string; readonly what: string; readonly moved: string }
/** The first use, in `list_uses` order, that holds before a drop of `moved` at gap `gap` (the used feature comes before its user) and not after it; null when none or when the uses are not current. */
export function dropWarning(state: AppState, moved: string, gap: number): DropWarning | null
/** The features in the document, other than `feature`, that have a use of it, in timeline order. */
export function usersOf(state: AppState, feature: string): readonly string[]
/** For the sketch `sketch`: each entity that a use names (`entity` set, with a `feature` that has a use of `sketch` without an entity, a feature link), mapped to its users in timeline order. */
export function usedEntities(state: AppState, sketch: string): Readonly<Record<string, readonly string[]>>
```

`what` names the used thing by the selection kind of the user's field that the use's `field` maps to (the mapping rule of 3.7, over the user's manifest hints): `edge` "an edge"; `face` "a face"; `plane` "a face" when the value at that field has `kind` `face`, else "a plane"; `axis` "an edge" or "a face" by the value's `kind`, else "an axis"; `sketch_region` "a region"; `sketch_point` "a point"; `sketch` "a sketch"; `body` "a body"; any other or none "a shape". `drop_breaks_use` reads `'{user}' uses {what} made by '{feature}'. Drop below '{feature}' instead?` when the dragged feature is the user, and `'{user}' uses {what} made by '{feature}'. Drop above '{user}' instead?` when it is the used feature.

Problems panel (side panel `problems`, `data-testid="problems-panel"`), opened by the problems button or the palette command "Problems". Its rows are grouped by root cause (`problemRows` in `src/problems/problems.ts`), so one cause is one problem:

- A feature whose status is warning or error gets a top-level row (`data-testid="problem-row"`, `data-feature-id`), in timeline order, unless it is nested (next item): the status icon, the feature label (a button that selects it), each of its warnings and errors with the message's actions, its cause line with the cause's actions, and Copy details when it has a `detail` (4.1, 4.2), then "Show what '{label}' lost" when its status has repairs, and "Edit" (`editFeature`).
- A feature whose first message with a `root` names another feature of the document (an `upstream_failed` chain, a `body_empty` whose body's maker failed, a `no_effect`, `hole_misses_body` or `separate_solid` under a sketch that lost its face or whose face turned over; recompute.md section 3.10) is listed under that root's row instead, indented (`data-nested="true"`), with its label, its messages without their actions, and the one action "Show '{root}'", a `show` of the root (4.2). A root whose status is neither warning nor error still heads its group with a row of its label and status icon.
- Document diagnostics follow, one row each.
- Info notices are not problems: every message of severity info is listed, with its feature's label and its actions (such as "Keep this match"), under a disclosure button "Notes" at the end (`data-testid="problems-notes"`, `aria-expanded` false until opened, the number of notes after the word in `textMuted`).
- While focus is inside a top-level row of a feature with repairs, repair mode shows that feature's repairs (3.7).
- With no rows: "No problems." (the Notes still follow).

The panel and the problems button count one problem per top-level row and per document diagnostic (`selectProblemCount`); nested rows and notes are not counted.

```ts
// src/problems/problems.ts
export type NestedProblem = { readonly feature: string; readonly root: string; readonly messages: readonly Diagnostic[] }
/** A top-level row: a feature (or a root that heads a group) with its warnings and errors, or a document diagnostic (`feature` null). */
export type ProblemRow = { readonly feature: string | null; readonly messages: readonly Diagnostic[]; readonly nested: readonly NestedProblem[] }
export type ProblemRows = { readonly rows: readonly ProblemRow[]; readonly notes: readonly { readonly feature: string; readonly message: Diagnostic }[] }
/** The rows of the Problems panel from the latest statuses and diagnostics, in timeline order (3.9). */
export function problemRows(state: AppState): ProblemRows
```

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
- Origin item: icon, name ("Top plane (XY)", "X axis", "Origin"), eye button ("Hide {name}" / "Show {name}"). An axis row's icon (`move-right`) is in its axis role's foreground, beside the name that carries its letter; the plane rows' icons (`square`) and the Origin row's icon keep their color, since a plane's name carries the letters of the two axes it holds ("Top plane (XY)"), not of its normal.
- Parameter: "{name} = {expr}" in the code face (`font-mono`, 2.16.7) and the evaluated value in `textMuted` with `tabular-nums` when the expression is not a plain number, on the baseline of the code (`items-baseline`; centering the two would set the smaller value higher); the group header has an icon button "Edit parameters" (`sliders-horizontal`).

Behavior:

- Rows map to selection items: body rows to kind `body`, sketch rows to `feature`, datum rows to `datum_plane` (key `feature:{id}`), origin rows to `origin_plane`, `origin_axis` or `origin_point`, parameter rows to `parameter`.
- Click selects (Shift or Mod toggles). While a feature command is active, clicking a row whose kind the active field accepts captures it instead (origin planes and datums for `plane`, origin axes for `axis`, bodies for `body`, sketches for `sketch`).
- Double-click or Enter: sketch: `enterSketch`; datum: `editFeature`; body: inline rename; parameter: opens the Parameters panel focused on it.
- F2 renames bodies (`update_body`, label "Rename") and features (as in the timeline). Delete removes selected sketches and datums (`deleteFeatures`) and selected bodies (`remove_body`, label "Delete {name}"; the core does not check use, so dependent features fail and the failure notification offers Undo). V toggles visibility of selected rows.
- Body eye: `update_body` with `visible` flipped (labels "Hide {name}" / "Show {name}"; undoable). Hidden bodies are not drawn or picked.
- Sketch, datum and origin eyes change `view.sessionVisible` only (not saved, not undoable). On opening a document, a sketch starts hidden when another feature's params contain an object whose `sketch` key equals its id (the `ProfileRef`, `AxisRef` and hole shapes of features.md 2.5), and visible otherwise; datum planes start visible; origin planes start hidden. The origin axes and the origin point are shown while the document has no features and hidden once it has one, so an empty view has an anchor without putting a pickable Z axis through most models. A `view.sessionVisible` entry, written only by their eye button or V, always wins (`selectOriginVisible(state, key)`). Origin planes are always shown while a plane is being picked.
- Lock: `view.locked` only (session); locked bodies are drawn but not pickable.
- Color: a popover (`data-testid="color-popover"`) with the swatch buttons of `BODY_SWATCHES` (2.16.5), named Default (removes `color`), White `#f4f4f5`, Black `#27272a`, Gray `#71717b`, Red `#e7000b`, Orange `#ff6900`, Yellow `#fdc700`, Green `#00a63e`, Teal `#009689`, Blue `#155dfc`, Purple `#9810fa`, Pink `#e60076` and Brown `#973c00`, and a hex field accepting `#rrggbb` (with `tabular-nums`). Choosing commits `update_body` with the color (label "Change color"). A body's current color (in its browser row and in the selection inspector) can be any hex from the document, so it is drawn as an inline SVG square whose `rect` has the stored hex (or `body`'s hex for no color) as its `fill` attribute, inside a `borderControl` border: document data known only at runtime goes into an SVG attribute, never a `style` (3.12).
- Keyboard: WAI-ARIA tree pattern (Up, Down, Right expands or enters, Left collapses or goes to the parent, Home, End), no type-ahead (letters stay global shortcuts). Row buttons are not tab stops; their actions are V (visibility), the selection inspector (color, material, visibility) and the palette commands "Lock selection" and "Unlock selection".

### 3.11 Parameters panel

The side panel `parameters` (`data-testid="parameters-panel"`, title "Parameters", close button) opens from the browser's "Edit parameters" button, a parameter row, or the palette command "Parameters".

- One row per parameter (`data-testid="parameter-row"`, `data-id`): name field ("Name of {name}"), expression field (`ExpressionInput` with unit `any` and `code`, "Expression of {name}"), both editable code (`font-mono` with `[font-variant-ligatures:no-contextual]`, 2.16.7), value (`data-testid="parameter-value"`: the `recomputed` value formatted by its dimension, with `tabular-nums`, or the error message in `errorText` with a `circle-x` icon in `errorMark`), comment field ("Comment of {name}"), delete button ("Delete {name}").
- `ExpressionInput` with unit `any` accepts every dimension and shows `= {value}` formatted by the returned dimension: length and angle as in 2.13, dimensionless as a scalar, any other dimension as `{value} mm^l rad^a` in internal units (zero exponents omitted, exponent 1 without `^`).
- Name: Enter or blur calls `renameParameter` when changed; the core rewrites every expression that uses the old name and labels the undo entry "Rename parameter {old} to {new}". Locally, a name that does not match `^[A-Za-z_][A-Za-z0-9_]*$` or is longer than 64 characters shows "Names start with a letter or _ and use only letters, digits and _." and is not sent; other rules (reserved names, uniqueness) come back as the core's error message under the field. Esc reverts.
- Expression: Enter or blur commits `update_parameter` (label "Change {name}") when valid and changed; Esc reverts.
- Comment: Enter or blur commits (label "Change comment of {name}"); empty removes the comment.
- "Add parameter" appends a draft row whose name field is focused; Enter in the name moves to the expression (default text empty); Enter in a valid expression commits `add_parameter` at the end (label "Add parameter {name}"); Esc discards the draft. A field's "Add parameter {name}" button (3.7) opens the same draft with the name and expression filled in.
- With no parameters, the panel shows above "Add parameter", in `textMuted` (`data-testid="parameters-empty"`): "Parameters are named values, such as wall = 2 mm. Type a parameter's name in any size or angle field to use it."
- During a feature command, parameter changes join its transaction and the panel says so (`parameters-transaction`, 3.7).
- Delete commits `remove_parameter` (label "Delete parameter {name}"). Expressions that used it then fail; the failure notification offers Undo.

### 3.12 Viewport rendering

- The canvas fills the window under the islands (3.2), and its projection is the free area's off-center projection (2.11); its drawing buffer is the CSS size times `min(devicePixelRatio, 2)`. Rendering is on demand: a frame is drawn only after a change to the camera, scene data, highlights, theme, size, section or analysis, batched with `requestAnimationFrame`. Nothing renders while idle.
- Shading is computed in linear space and written as sRGB.

Passes, in order:

1. Background: the `viewport` color (2.16.4), one flat tone.
2. Opaque bodies (model layer, and preview layer during a command): matcap-style shading from the view-space normal `n`: `k = 0.32 + 0.50 × max(dot(n, L), 0) + 0.18 × max(n.z, 0)` with `L = normalize(0.3, 0.6, 0.74)`, color `base × k + 0.06 × (1 - max(n.z, 0))^3` (rim). `base` is `Body.color` or `body`. Element states mix the result toward `highlight` (hover 0.20, selected 0.35) or `problem` (problem 0.5), mixing the sRGB-encoded colors and linearizing the result, as pass 3 mixes the ramp, so a selected face keeps the accent's hue on screen (mixed in linear light, the default body's selection tint turned rose, the hue of `axisX`). Faces use a polygon offset (factor 1, units 1). With a section, back faces are drawn flat in `base × 0.55` (the cut cap). Bodies whose role is not `part` draw at 40 percent opacity in the translucent pass.
3. Overhang map (Print context, 3.17): for a vertex value `v` (radians from vertical) above the limit `a`, `t = clamp((v - a) / (π/2 - a), 0, 1)` the ramp color is `ramp = linearize(mix(analysisLow, analysisHigh, t))`, mixing the sRGB-encoded ends (as the legend's `bg-linear-to-r/srgb` gradient does) and linearizing the result, and the color becomes `mix(shaded, ramp × k, 0.85)`.
4. Grid, then edges: first the grid (below), then B-rep edges from `edge_positions` as screen-space quads (instanced per segment), 1.25 CSS pixels wide (times the UI scale), anti-aliased with a 1-pixel smoothstep falloff, depth-tested against the faces, in `edgeDark` or `edgeLight` per body (2.16.4). Selected and hovered edges as in 3.6. The grid comes after the faces, which hide it, and before the edges, because neither writes depth: drawn after them, a grid line that coincides with an edge (the bottom edges of every part standing on the ground at whole millimeters) would paint over it.
5. Vertices: only hovered and selected vertices are drawn.
6. Translucent: datum and origin planes as squares with their color (below) as fill at 15 percent and outline at 60 percent; non-part bodies.
7. Overlay (no depth test): sketch curves, points and region fills (3.16.2); in Model context visible sketches draw in `sketchReference` at 1.25 CSS pixels with no region fill, except hovered or selected regions (`highlight` at 20 or 35 percent) and, while the active selection field accepts `sketch_region`, every region of every visible sketch in `sketchRegion` at 12 percent (20 percent when hovered), so the shapes that can be clicked show. The helper sketch points of a Hole command (3.7) draw here too, and so do the split planes of the focused print finding (3.17), so the cut shows through the body.
8. Print bed outline (depth-tested, 3.17).

- Grid: in Model context, lines on the XY plane (z = 0); in sketch mode, lines on the sketch plane through the placement origin along the placement axes instead (on the XY plane while a plane is picked for a new sketch, which has no plane yet); none in Print, which draws the bed outline, and none while the grid is turned off (the palette toggle "Show grid", `ViewState.grid`). The minor spacing is `gridSpacing` (2.11) at the camera target; every tenth line is major. The grid covers a square of side `max(200 mm, 2 × the diagonal of the visible bounds)` centered on the origin (or the placement origin), rounded out to whole major cells. Lines are 1 CSS pixel wide (times the UI scale) in `grid`, minor at 35 percent and major at 70 percent opacity, depth-tested so bodies hide them; they are not drawn into the ID pass and are never snap targets. The controller mirrors the drawn grid's minor spacing in `view.gridSpacing` whenever it changes (2.8); the viewport shows it in the document unit (`formatNumber`) in `data-grid-spacing`, empty when no grid is drawn, and the status bar shows it (3.21), so the grid gives scale and not only orientation.

- Datum and origin planes are drawn as squares of side `max(50 mm, 1.2 × the diagonal of the visible bounds)` centered at the projection of the visible bounds center onto the plane. Every drawn origin plane (all three while a plane is picked or a plane field is active, otherwise those shown this session) carries its plane label of 3.16.1 at its corner. Origin planes take the color of their normal's axis (Top plane (XY) `axisZ`, Front plane (XZ) `axisY`, Right plane (YZ) `axisX`), as the owner asked, and are named by their plane label rather than a letter; datum planes take `datum` (2.16.4). Origin axes are drawn as dashed lines of the same length through the origin, 1.25 CSS pixels wide (times the UI scale), each in its axis role (`axisX`, `axisY`, `axisZ`; one draw with per-element colors), with DOM labels "X", "Y", "Z" at their positive ends (`data-testid="axis-label"`, `data-axis`, `font-semibold` in the axis role's foreground on their `surface` chip). Hovered and selected origin axes, and the outlines of hovered and selected origin and datum planes, are drawn 2.5 CSS pixels wide (times the UI scale) in `highlight`, as edges are (3.6), so hover and selection change the stroke as well as the color at every UI scale.
- The overlay layer (`absolute inset-0 overflow-hidden pointer-events-none` over the canvas, so it fills the window under the islands like the canvas, 3.2, and an overlay element under an island is covered by it; interactive children `pointer-events-auto`) holds an SVG (`absolute inset-0 size-full`) for the box selection rectangle, profile issue rings, finding markers, repair ghosts and markers (3.7) and the cause ghost of a `show` (4.2), and HTML elements for sketch dimension labels, constraint glyphs, draw inputs, the dimension editor, inference glyphs, axis labels, plane labels and repair candidate markers. Their positions follow the runtime positions rule below and update in the same animation frame as the canvas.
- WebGL context loss: `preventDefault` on `webglcontextlost`, a status overlay "Restoring the 3D view" (`role="status"`), and on restore `scene.restore()` re-uploads every drawable and renders.

Runtime positions, the one exception to the no-inline-style rule (its scope: open issue 14):

- Which: the overlay's HTML elements above (plane labels and repair candidate markers included, 3.16.1, 3.7); the inspector (3.7); the timeline's drag ghost, insertion bar and drop hint (3.9); menus, popovers and tooltips anchored to their triggers and the name suggestions of expression fields anchored to their inputs (2.13) (the native Popover API places them in the top layer, and Tailwind 4.3.3 has no anchor positioning utilities), and the viewport context menu anchored at the pointer (3.13); and the view cube's rotation (3.14). The overlay's SVG marks use SVG geometry attributes instead (`x`, `y`, `width`, `height`, `cx`, `cy`, `r`), and `d` for the paths of ghosts, with colors from stroke classes (`accent`, `errorMark`) and the dashes of the crossing selection, the repair ghosts and markers and the cause ghost as the static attribute `stroke-dasharray="4 3"`.
- Why: these values come from projecting 3D geometry through the camera every frame (or from the pointer during a drag, or from a trigger's measured rectangle), so no class known at build time can express them, and re-rendering React every frame would break the rule that the viewport bypasses React (ADR 0002).
- How: React renders each element once with its classes, test ids and accessible names and never passes a `style` prop. The viewport controller, or the owning component through a ref, writes the position imperatively, through the two functions of `src/viewport/overlay.ts`, which are the only code in `src/` that touches `element.style`:

```ts
/** Moves an element to (x, y) CSS pixels from its layer's origin: sets only `element.style.translate` (`${x}px ${y}px`) and marks the element `data-positioned`. */
export function positionElement(element: HTMLElement, x: number, y: number): void
/** Sets only `element.style.transform` to the CSS `matrix3d` of a column-major 4 × 4 matrix and marks the element `data-positioned`. */
export function orientElement(element: HTMLElement, matrix: Float32Array): void
```

- Positioned elements sit at `absolute left-0 top-0` of their layer. The inline `translate` replaces any translate class on the same element, so an element centered on its anchor wraps its content in an inner element with `-translate-x-1/2 -translate-y-1/2`. An element whose anchor is behind the camera or outside the free area (3.2) gets the `hidden` attribute instead of a position, so no focusable glyph, label or input sits under an island. An axis or plane label whose box meets a card or chip of the free area (each marked `data-card`: the view cube and its Home view button, the viewport controls, the overhang legend, the empty-state card, the sketch header and the sketch hint, the print hint, the inspector and the core panels) is hidden too, since half under a card of its own tone it would read as part of it; `hideLabelsUnderCards` (`src/viewport/overlay.ts`) reads every box before it hides any, so a frame computes the layout once. The draw inputs and the dimension editor (3.16.4, 3.16.7), which take typed values, stay inside the free area: the sketch controller reads their `offsetWidth` and `offsetHeight` and clamps their position so their box keeps 0.5rem inside it. SVG geometry attributes are written with `setAttribute` in the same frame.
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
| Zoom with a trackpad pinch | At the cursor | At the cursor |
| Fit to view | Middle double-click | None |
| Orbit with the view cube | Left drag on the cube | Left drag on the cube |
| Box selection | Left drag on empty space, sketch mode only | Left drag on empty space, sketch mode only |

- A gesture starts after 3 CSS pixels of motion; below that, a press and release is a click. Orbit rate 0.4 degrees per CSS pixel about the pivot: the world point under the cursor at the gesture start (from the ID pass depth) or the target when nothing is there or the backend cannot read depth.
- A trackpad pinch arrives as a wheel event with `ctrlKey` and zooms as the wheel does. The table is the keyboard and mouse panel's data (`NAVIGATION_PRESETS` rows, 2.11, 3.5), so its cells are written for users.
- Wheel uses `wheelZoomFactor`; one notch with a pixel delta of 100 zooms out by 1.0015^100 ≈ 1.162 (wheel back) or in by the inverse (wheel forward). Wheel listeners are non-passive and call `preventDefault`.
- Middle-button `pointerdown` and `mousedown` call `preventDefault` (no browser autoscroll). The canvas `contextmenu` event always calls `preventDefault` (on macOS and Linux it fires on press, before a right drag can start). A right-button press that moves 3 CSS pixels or more is a camera gesture (the threshold above); a right press released before that opens the viewport context menu.
- Viewport context menu (`data-testid="viewport-menu"`, `role="menu"`), at the pointer; a pick target under the pointer that is not selected first becomes the selection (as a click would). Shift+F10 or the ContextMenu key on the focused viewport opens it at the center of the selection's screen rectangle, or the free area's center. Items come from `viewportMenuItems(app)` (2.11), each with its formatted shortcut, leaving out disabled ones, so the menu and the radial marking menu of 1.2 can share them:
  - A feature command active: "OK" (when complete) and "Cancel".
  - Sketch mode: the applicable constraints of the sketch selection (3.8), "Construction" and "Delete" when something is selected, a separator, "Finish sketch".
  - Otherwise: "Repeat {title}" (`edit.repeat`, when there is a command to repeat), the selection's suggested commands (3.8), "Edit {label}" (one face, edge or vertex: its `element.feature`; one feature: that feature; never an `import`), "Hide" (`view.visibility`, when the selection holds bodies, sketches, datum planes or origin items, which have visibility; "Show" when all of them are hidden), "Look at" (when enabled), a separator, "Fit to view".
  - A right click selects nothing during a feature command, a Lay on face pick, plane picking or an active sketch tool; the menu still opens.
- Islands (3.2) lie above the canvas: a pointer over an island is not over the canvas, so moving onto one ends the hover (the canvas's `pointerleave` sets the hover to none), and no click, hover, wheel or right-click over an island picks, selects, zooms or opens the viewport menu. A press that started on the canvas keeps its pointer capture over an island, so an orbit, pan or box selection carries on there; the gaps between islands are canvas.
- During a gesture hover picking pauses; the ID pass is re-rendered on the next pick after the camera has been still for 100 ms.
- The viewport is focusable (`tabindex="0"`, `role="application"`, `aria-roledescription="3D view"`) and takes the keys of 3.5.
- The navigation preset is a setting (`settings.navigation`), mirrored on the viewport as `data-nav-preset` (`fusion` or `freecad`). The right-drag gestures work in both presets, so no one has to pick a preset to orbit; the preset only decides the middle-button gestures.
- The viewport mirrors the camera in `data-view` (`classifyView`), `data-projection`, and `data-camera-target` (`"{x},{y},{z}"` with each coordinate rounded to 2 decimals, trailing zeros trimmed, `-0` as `0`).

### 3.14 View cube and viewport controls

- The view cube (`data-testid="view-cube"`, `role="group"`, `aria-label="View cube"`) is a DOM element in the free-area layer (3.2), a `pointer-events-auto` fieldset, `size-24` (6rem, so it follows the UI scale) at `top-11 right-9` of the free area, its top-right corner, far enough from the top and the right edge that its corners stay inside the free area with 0.7 to 1.2rem to spare, as the controls' card keeps 1rem in the other corner, and never reach under an island. In the views of 2.11 its corners lie up to 4√(3/2) rem (4.90rem) above and below the cube's center and 3√2 rem (4.24rem) beside it, a little more for the near corners under the cube's perspective (`perspective-distant`, 1200 CSS pixels at any UI scale): in the iso view the near side corner reaches 4.34rem beside the center at 100 percent UI scale, 4.45rem at 200 and 4.56rem at 300 percent, and the near bottom corner 5.02, 5.14 and 5.26rem below it. So the cube reaches at most 9.81rem from the free area's right edge and 11.01rem below its top, which the top-center slot keeps clear of (3.2); with `top-8 right-6` the iso view's corners came within 0.2rem of the free area's top and passed its right edge at 300 percent. Its static 3D layout uses Tailwind's 3D transform utilities only (`perspective-*`, `transform-3d`, `backface-hidden`, `rotate-x-*`, `rotate-y-*`, `translate-x-*`, `translate-y-*`, `translate-z-12`; translations are in the parent's frame, since Tailwind applies `translate` before `transform`), and an inner rotor turns every frame to match the camera through `orientElement` (3.12). Faces are `surface`, carried by each face element under its cells, with `textMuted` labels, and the cells of the hovered face, edge or corner target take `hover`'s `active` fill over it. Their sides are drawn in the colors of the axes they run along, 2 CSS pixels wide (Tailwind's `border-2`), by two `aria-hidden` `pointer-events-none` overlays over each face, one `absolute inset-0 border-y-2` and one `absolute inset-0 border-x-2`, each with one axis role's `border` facet, so the face's cells, and with them every target, keep their size: the Top and Bottom faces' horizontal sides in `axisX` and vertical sides in `axisY`, the Front and Back faces' in `axisX` and `axisZ`, the Left and Right faces' in `axisY` and `axisZ`. Every edge of the cube is thus drawn in its axis's color by both faces that meet there, so the cube shows which way X, Y and Z run in every view (it is the axis triad; there is no other), and the face labels say the same without color. Its 6 faces are buttons labeled "Top", "Bottom", "Front", "Back", "Left", "Right" with accessible names "{Face} view"; its 12 edges are buttons named "{Face} {face} edge view" (pairs in the order Front, Back, Left, Right, Top, Bottom: "Front top edge view", "Front left edge view", "Right bottom edge view", ...); its 8 corners are buttons named "{Front or Back} {Left or Right} {Top or Bottom} corner view".
- Clicking a face sets that standard view; an edge sets the view looking from the normalized sum of its two face directions; a corner from the sum of three (Front right top is `iso`). Up is the projection of +Z onto the view plane, except views looking straight down or up (top and bottom faces) which use the up vectors of 2.11. The target, distance and projection are kept; the change animates.
- Dragging on the cube orbits about the target at the same rate as the navigation orbit.
- A "Home view" button (`house` icon) outside the cube's top-left corner sets `iso` and fits the visible bounds. It is a chip that stays over the viewport, `size-6 rounded-md border shadow-md` in `surface`, `forcedEdge` and `elevation` with a `textMuted` icon, and takes `hover`'s `opaque` facet on hover (2.16.3). It sits at `-top-7 -left-7` of the cube's box, 1rem below the free area's top and 8.5rem to 10rem from its right edge, so the top-center slot's column (10.5rem from that edge, 3.2) keeps 0.5rem from it, and it covers no face, edge or corner target in the standard, edge and corner views: its nearest corner is 4.6rem from the cube's center, while the turned cube's outline comes at most 4.5rem from the center in that direction.
- Viewport controls (`data-testid="viewport-controls"`), a vertical stack of `size-8` `rounded-md` icon buttons in a card (`pointer-events-auto flex flex-col gap-1 rounded-xl border p-1 shadow-md`, `surface`, `forcedEdge`, `elevation`) at the free area's top-left corner: the first child of the free area's left column (`pointer-events-none absolute inset-y-4 left-4 flex flex-col flex-wrap content-start items-start gap-2`), whose last child is the overhang legend (`ViewportProps.legend`, 3.17) with `mt-auto`. In a free area tall enough for both, the legend sits at the column's bottom under the controls; in a shorter one it wraps into a second column right of the controls, still at the bottom, so it never covers them and never goes under the timeline (at 200 percent UI scale on a 1280 by 720 window the column is 9.75rem tall, the card 9.25rem and the legend about 7.5rem). The cube and the controls take the two top corners, so the top-center slot between them keeps clear of both (3.2); the stack (its card 9.25rem tall, ending 10.25rem below the free area's top) fits beside the cube in a free area 11.75rem tall (200 percent UI scale on a 1280 by 720 window), where it could not fit under it. The buttons: "Fit to view" (`maximize`), "Orthographic" (`cuboid`, `aria-pressed` true in orthographic), "Section view" (`scan-line`, `aria-pressed`; turning it on opens the section popover), "Selection filter" (`mouse-pointer-2`, a menu of `menuitemcheckbox` items Bodies, Faces, Edges, Vertices; hidden in sketch mode).
- Fit to view frames `scene.visibleBounds` with `fitBounds` (margin 0.1) in the free area, or in its part beside an open inspector (2.11), animated; with nothing visible it frames a 100 mm cube at the origin.
- Look at (`view.look_at`): with one flat face (its `placement`), origin plane or datum plane selected, or in sketch mode (the sketch placement), the camera looks along the negated normal from the front side with the plane's y-axis up, target the plane point (face centroid, plane origin), distance and projection kept, the target at the center of the free area or of its part beside an open inspector (2.11). Otherwise the command is disabled with "Select a flat face or a plane first."
- Orthographic toggle uses `withProjection`.

### 3.15 Section view

- Toggled by the "Section view" control or Shift+X. When turned on, the section popover (`data-testid="section-popover"`) opens anchored to the control: Plane (segmented XY, XZ, YZ; default XY), Offset (`ExpressionInput`, unit length, default the center of the visible bounds along the plane normal formatted with `formatNumber`), Flip (switch).
- The section removes the half-space where `dot(n, p) > offset`, with `n` the plane normal (XY +Z, XZ -Y, YZ +X; Flip negates both `n` and `offset`). It clips bodies, edges, vertices and the ID pass (removed parts cannot be picked). Sketches, datum and origin planes and the bed are not clipped.
- The section is a view setting: not saved, not undoable, kept across context changes until turned off. `data-section` on the viewport is `on` or `off`.

### 3.16 Sketch mode

#### 3.16.1 Entering

- `startNewSketch`: when the selection is exactly one plane-like item (origin plane, datum plane, or a face whose `surface` is `plane`), `createSketchOn(item)`. Otherwise the app enters plane picking: context Sketch with `sketchId` null; the sketch header reading "Pick a plane or a flat face to draw on" followed by the buttons "Top", "Front" and "Right" (accessible names "Sketch on Top plane (XY)", "Sketch on Front plane (XZ)", "Sketch on Right plane (YZ)"), which run `createSketchOn` for that origin plane, so the first plane is one click or one Tab away (the header is `max-w-full` of the slot and wraps, `flex-wrap` with the prompt free to wrap and the three buttons in one `flex gap-2` group that wraps as a whole, so in a narrow slot, as at 200 percent UI scale with the browser open, the prompt and the buttons take more lines instead of running out of the viewport, and the buttons never split across lines); origin planes shown, each with a label at the corner of its square in the plane's positive first and second axis directions (`data-testid="plane-label"`, `pointer-events-none`, `text-xs` on a `surface` chip, positioned per 3.12): "Top (XY)", "Front (XZ)", "Right (YZ)"; the sketch hint (3.16.3) reading `plane_pick_hint` ("Top plane (XY) is the print bed."); and only origin planes, datum planes and faces accepted. A face that is not flat shows the hint `pick_plane`. Clicking a browser origin plane or datum row works too (other rows select as usual). Esc returns to Model.
- `createSketchOn(item)`: commits one `apply_edits` labeled "New sketch" with `add_feature` of a `sketch` record whose `plane` is sketch.md's `SketchPlane` for the item: `{"kind": "origin", "plane": "XY"}`, `{"kind": "datum", "feature": id}` or, for a face, `{"kind": "face", "body": id, "face": <Reference>, "fallback": <Placement>, "fallback_frame": "body"}` with the face's full reference and, as the fallback, its `ElementDescriptor.fallback` (the placement in body coordinates, engine.md section 3.9), so the sketch keeps its place on the face when a Move body or a copy moves the face later (features/sketch.md section 2); empty entities and constraints; inserted by the marker rule of 3.7. Then `enterSketch(new id)`. While one `createSketchOn` runs, another does nothing, so a double-click on a plane (or on Sketch with a plane selected) adds one sketch.
- Re-attaching (`reattachSketch(id)`, run by "Show what '{label}' lost" and by a `pick` action at `/plane` or `/plane/face` of a sketch, whose plane has no inspector field): the plane picking above, with the header prompt `reattach_prompt` ("Pick a plane or a flat face for '{label}'", 4.3) and the same plane buttons, while repair mode shows the sketch's repair (3.7: the lost face's ghost and the numbered candidates, whose buttons are listed in the header under the prompt as "{n}. {Description}" with the relation, and run their `Attach to {description}` action). A pick commits one `apply_edits` labeled "Attach '{label}'" with an `update_feature` of the sketch whose `plane` is built as `createSketchOn` builds it (a face gives its full reference and its descriptor's `fallback` with `fallback_frame: body`), except that a sketch without motion (on an origin plane, or on a face whose fallback is in world coordinates, as every face sketch saved before format version 2) stores a face's descriptor `placement` in world coordinates without `fallback_frame`, which keeps it without motion, so the faces built from it and the fallbacks and directions later features store on them stay in place (engine.md section 3.22, open issue 25); then `enterSketch(id)`. A plane or face that a feature after the sketch in the timeline made (the item's `feature`: a face's descriptor `feature`, or the datum plane) is refused with `attach_after` in the sketch hint and the pick stays open, since the sketch is built before it. An `apply` action that leaves the sketch with no repair in its plane, such as a candidate's, is followed by `enterSketch(id)` too; one that does not apply (`stale_fix`) leaves the pick open. Esc returns to Model with nothing changed. The entities keep their sketch coordinates, so the drawing lands on the new plane as it lay on the old one.
- `enterSketch(id)`: cancels an active command, waits for sketch commits still queued when another sketch is being edited, sets context Sketch, clears the app selection (the plane or face the sketch started on has done its job, so a feature command run from the sketch preselects what the sketch made, 3.3), shows the sketch for the session, gets its solution (`sketch_solved` events or `solveSketch(documentHandle, id)`), and moves the camera: `lookAlong` the placement normal with the placement y-axis up, then, when the sketch has entities, frames their bounds in the sketch plane (`fitBounds`, margin 0.1), together with the sketch origin when one of its constraints refers to the origin or an axis (sketch.md's reserved `origin`, `x_axis` and `y_axis`), so that axis and a dimension to it are in view; an empty sketch keeps the visible height and targets the placement origin. The projection is kept. The camera before entering is pushed as a sketch view entry (3.20) and kept as `cameraBefore` (3.16.11); entering another sketch while one is edited keeps the session's `cameraBefore`, which still looks at the model. Downstream features stay visible and recompute after each sketch edit.

#### 3.16.2 Display

- Active sketch curves: overlay lines 1.5 CSS pixels wide (times the UI scale) colored by `SolvedEntity.constrained` (sketch.md `EntityState`): `under_constrained` `sketchUnder`, `fully_constrained` `sketchFull`, `conflicting` `sketchOver` (2.16.4). Construction entities are dashed (6-pixel dashes, 4-pixel gaps) and drawn in `sketchConstruction` instead of `sketchFull` when fully constrained, so finished construction geometry recedes behind the profile; under-constrained and conflicting construction entities keep `sketchUnder` and `sketchOver`. Hovered and selected curves are 2.5 CSS pixels wide (times the UI scale) in `highlight`. Circles and arcs are tessellated by the UI from entity data with a chord tolerance of 0.25 CSS pixels, re-tessellated when the scale changes by more than a factor 1.5.
- Points: line ends and arc ends as 5-pixel squares and centers as 5-pixel crosses in their entity's color; point entities as 6-pixel filled circles; points shared by a coincident constraint as filled squares; hovered and selected points 7 pixels in `highlight`.
- The origin point (9-pixel cross, `sketchReference`) and the sketch's X and Y axes (dashed lines across the view): on an origin plane each axis in the role of the world axis it lies on (Top plane (XY): `axisX` and `axisY`; Front plane (XZ): `axisX` and `axisZ`; Right plane (YZ): `axisY` and `axisZ`; architecture.md's origin plane frames), and on a datum plane or a face both in `sketchReference`, since their axes are not world axes; pickable for snapping and constraints, which refer to them by sketch.md's reserved references `origin`, `x_axis` and `y_axis` (2.14). Hovered and selected sketch axes are 2.5 CSS pixels wide (times the UI scale) in `highlight`, as curves are. On an origin plane each sketch axis carries the `axis-label` chip of its world axis (3.12; the world's origin axes and their labels are not drawn while a sketch is edited), placed where the axis's positive half leaves the free area, 1rem inside the free area's edge, then moved 0.75rem (times the UI scale) off the axis to its negative side (below the X axis, left of the Y axis) and kept 1rem inside that edge, so the chip never sits on the dashed line or on geometry drawn from the origin into the positive quadrant; it is hidden while that half does not cross the free area, and a sketch on a datum plane or a face has none.
- Closed regions filled with `sketchRegion` at 12 percent from the solution's display buffers; hovered regions at 20 percent.
- Profile issues: each `profile_findings` entry gets a ring of 12 CSS pixels diameter stroked in `errorMark` (an SVG `circle` in the overlay, `data-testid="profile-issue"`, with an SVG `title` naming the issue): `open_endpoint` at `point` ("Open end"), and one with `connect` also at the solved position of its `connect.second` point (it stands for two open ends, sketch.md section 4.3), `overlap` at the midpoint of `start` and `end` ("Overlapping curves"), `self_intersection` at `point` ("Curves cross"); `zero_length` gets no ring; the entities named by `entity` or `entities` get a 4-pixel `problem` halo at 30 percent in the WebGL overlay pass. The header and the finishing notification call these warnings, because the sketch stays valid and its other shapes still extrude; the rings stay `errorMark` because they mark exact spots and a yellow ring would not reach 3:1 against the viewport (2.16.6). The halo lies under the curve, which keeps its DOF color: it is a band 4 CSS pixels (times the UI scale) wider than the curve on each side, drawn before the curves in `problem` mixed 30 percent over `viewport` (sRGB-encoded, as blending would mix it) and opaque, because the overlapping segment quads of a tessellated circle or arc would darken a translucent band in beads. Tinting the curve itself would turn an under-constrained blue line violet, a color that means nothing (2.16.3).
- Other visible sketches draw in `sketchReference` and are not pickable in sketch mode.
- Constraint glyphs (when "Show constraints" is on, the default; palette command "Show constraints" toggles it): one DOM button per geometric constraint except `coincident` and `point_on_object` (which have no glyph; the shared point is drawn filled), `size-4.5` (so it follows the UI scale), `data-testid="constraint-glyph"`, `data-constraint-id`, `data-type`, `data-state` (`satisfied`, `redundant`, `conflicting`, `unsatisfied`), accessible name the constraint title. Placement (runtime positions, 3.12): anchored at the midpoint of the first constrained curve (lines), the point at 45 degrees on circles and arcs, or the point itself, offset 14 CSS pixels (times the UI scale) perpendicular to the curve on screen; further glyphs on the same anchor stack 20 pixels apart along the curve. Style: a `rounded-sm` chip with a 1-pixel `border` in `forcedEdge` (`accent` while the constraint is selected); `satisfied` and `unsatisfied`: icon in `textMuted` on `surface` at `opacity-90`; `redundant`: `warningFill`; `conflicting`: `errorFill`. Hovering highlights the constrained items; clicking selects the constraint.
- Dimensions: dimension geometry (extension lines, dimension line with 8-pixel arrowheads, radius or diameter leader, angle arc) in `sketchReference` at 1 CSS pixel, plus a DOM label button (`data-testid="sketch-dimension"`, `data-constraint-id`, `data-type`, `data-driving`) at the label position (stored by sketch.md if it stores one, else `defaultLabelPosition`; positioned per 3.12). Label text, with `tabular-nums`: `formatNumber` of the solved value without unit; prefixed `⌀` for diameter and `R` for radius; suffixed `°` for angles; a non-plain-number expression is prefixed `fx ` (`fx 40`); reference dimensions are wrapped in parentheses (`(40)`). A `rounded-sm` chip with a 1-pixel `border` in `forcedEdge` (`accent` while the dimension is selected), as a constraint glyph has: `surface` at `opacity-90` with `text`; `errorFill` when conflicting, `warningFill` when redundant. Double-click or Enter opens the dimension editor.

#### 3.16.3 Tools and commits

- Tools are activated by the toolbar, shortcuts or the palette. Activating another tool cancels the current operation. The active tool's toolbar button has `aria-pressed="true"`. The cursor is a crosshair while a drawing tool is active.
- The pointer maps to sketch coordinates by intersecting the camera ray with the sketch plane; when the ray is within 1e-6 of parallel to the plane, pointer input is ignored.
- `commitSketchChange(label, change)` belongs to the sketch edited when it is called and runs as one task of the mutation queue (it may wait there behind a slow rebuild):
  1. Builds candidate params: the entities and constraints that sketch has when the task runs, plus the change.
  2. Calls `solveSketch(documentHandle, sketch, candidate)`. When a `requested` constraint id is in `conflicting` or `redundant`, returns `over_constrained` and applies nothing. When `inferred` ids are, removes them and solves once more.
  3. Calls `applyEdits(documentHandle, label, [update_feature])` with the candidate params whose entities carry the solved geometry (each `SolvedEntity` without `constrained`). The `plane` is kept as stored: only an attach, a committed drag and the core's fixes write a face sketch's fallback (features/sketch.md section 2), because a fallback in body coordinates (`fallback_frame: body`) cannot be computed from the world placement of `solution.placement` without the core.
  4. A solution with `failed` true, or an `invalid_params` or `expression_error` `CoreError`, returns `failed` with the solution's first message of severity `error` (`sketch_not_solved`, sketch.md section 4.2, in the document units) or the core's message; nothing is applied.
- Labels: "Line", "Rectangle", "Circle", "Arc", "Point", the constraint title, "Dimension", "Change dimension", "Construction", "Delete". Drags are committed by `end_sketch_drag` (the core labels them).
- Step prompt: the sketch hint (`data-testid="sketch-hint"`, `role="status"`, a `surface` chip with `text-xs` text in `textMuted`, under the sketch header in the top-center slot of 3.2) shows `sketchStepHint(step, platform)` (2.14): one sentence for the current step, from the `sketch_step_*` messages (4.3), with shortcuts rendered by `formatShortcut`. With no tool active it shows `sketch_step_start` in a sketch without entities and nothing otherwise, so a prompt appears only while a tool is active or the sketch is empty. A refused pick, a constraint tool's `constraintRequirement`, `arc_collinear` and `sketch_leave` (3.5) replace it while they apply. While an inference glyph is visible (3.16.5), the chip appends a space and `sketch_free_placement` in a span with `aria-live="off"`; the live region announces the hint only when the step changes, never on pointer moves. The hint element stays mounted at one place from outside sketch mode (empty and `sr-only`) through plane picking and editing, so it exists before its first prompt and every step's prompt is announced.

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

Each tool's output is fixed (unit tests U-50 to U-54 check `recipes.ts`; features/sketch.md tests 44 to 50 hold each tool's output as a document, kept in step by hand when a tool changes, since no UI test reads them); "inferred" constraints come from 3.16.5; everything else is requested.

- **Line / polyline** (`line`, L). Click 1 sets the start S. A rubber band follows the cursor. Click 2 sets the end E and commits a line S to E; segments shorter than 2 CSS pixels are ignored. The chain continues from E: the next line starts at E with an inferred `coincident` between the previous line's end and the new line's start. The chain ends on Esc (the rubber band is discarded, the tool stays), Enter with no draw input focused, a double-click (which commits the segment to the clicked point first), or a click on the chain's first point (committing a closing segment with `coincident` to it).
- **2-point rectangle** (`rectangle_2point`, R). Click 1 sets corner C1, click 2 the opposite corner C2; a width or height under 2 CSS pixels is ignored. Output: lines L1 C1 to (C2.x, C1.y), L2 (C2.x, C1.y) to C2, L3 C2 to (C1.x, C2.y), L4 (C1.x, C2.y) to C1; `coincident` L1.end-L2.start, L2.end-L3.start, L3.end-L4.start, L4.end-L1.start; `horizontal` L1 and L3; `vertical` L2 and L4. Snaps at C1 apply to L1.start and at C2 to L2.end (inferred).
- **Center rectangle** (`rectangle_center`). Click 1 sets the center P, click 2 a corner K. Output: the 2-point rectangle with C1 = 2P - K and C2 = K, plus a construction line D from L1.start to L2.end with `coincident` D.start-L1.start and D.end-L2.end, and a construction point Q at P with `midpoint` (Q, D). Snaps at P apply to Q and at K to L2.end (inferred).
- **Circle** (`circle`, C). Click 1 sets the center, click 2 a point on the circle (radius under 2 CSS pixels is ignored). Output: one circle. A snap at the center applies to its center point; a point snap at click 2 adds `point_on_object` (that point, the circle) (inferred).
- **3-point arc** (`arc_3point`). Click 1 sets A, click 2 sets B, then the arc follows the cursor T through A, B and T; click 3 commits. When `circleThroughThreePoints` returns null or a radius above 1e6 mm, the rubber band is a straight segment, click 3 is ignored, and the sketch hint (`data-testid="sketch-hint"`, `role="status"`) says "These three points are in a line. Move the last point off the line to make an arc." Arcs are stored counterclockwise: when T lies on the clockwise path from A to B, the arc runs from B to A. Snaps at A and B apply to the arc's ends; a tangent inference at A applies.
- **Point** (`point`). Each click adds a point entity at the (snapped) location.
- **Construction** (`construction`, X). With entities selected and no drawing tool active, toggles their `construction` flag in one commit labeled "Construction". With a drawing tool active, toggles construction mode for new entities (`aria-pressed` on the button). The mode resets when sketch mode ends.

Dimension as you draw (draw inputs: DOM text inputs next to the rubber band's midpoint, on its side away from the pointer so they never cover the next click, `data-testid="draw-input"`, accessible names as listed):

- Line: "Length" (unit length) and "Angle" (unit angle, counterclockwise from the sketch +X axis). Rectangles: "Width" (along X) and "Height" (along Y). Circle: "Diameter". Arc, after click 2: "Radius".
- While a shape is rubber-banding, each empty draw input shows the live measured value as its placeholder (`formatNumber`, with the input's `tabular-nums` and the `textMuted` placeholder facet), written by the sketch controller with `setAttribute('placeholder', ...)` in the same frame as the rubber band (never through React, 2.1), so typed entry can be seen. Typing a digit, `.`, `-` or `(` focuses the first draw input and inserts the character; Tab focuses the first input, then cycles between the inputs; once a draw input has focus every key types into it, so an expression that starts with a name (`width / 2`) is typed after Tab. Letters with no draw input focused stay tool shortcuts. A valid typed value locks that quantity (the rubber band follows it; the other quantity still follows the cursor). Esc in a draw input clears the inputs and returns focus to the canvas.
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
- Editing an existing dimension (double-click its label, Enter on the focused label, or the selection inspector) opens the editor prefilled with the stored expression text and clears the sketch selection, so the selection inspector (3.8), which edits the same value and floats over the overlay layer, does not open over the editor; Enter commits a replacement labeled "Change dimension". When the probe reports a non-empty `conflicting` list or `failed` true, nothing is applied, and the editor keeps the text and shows, with `role="alert"`, the probe's message (`SketchCommitResult.message` or the `failed` message, 3.16.3), the sentence the sketch would report once stored, such as `'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape.` (newcomer error corpus NC08), with the buttons "Add reference" (commits the replacement with `driving` false) and "Cancel".

#### 3.16.8 DOF display

- Entity colors come from `SolvedEntity.constrained` (3.16.2).
- The sketch header (`data-testid="sketch-header"`, a `surface` chip button (`hover`'s `opaque` facet on hover, 2.16.3) in the top-center slot of the free area (3.2), `data-state`, `data-dof`) shows the sketch label (`shrink-0`, truncated beyond `max-w-40`) and a status in plain words (`min-w-0`, truncated with an ellipsis when the slot is narrow, as at 200 percent UI scale). `data-state` is the first that applies: `empty` "No geometry yet" (no entities); `conflict` "{n} conflicting constraints" in `errorText` (`conflicting` non-empty); `failed` "Cannot solve" in `errorText` (`failed` true); `full` "Fully constrained" (`dof` 0); `under` "Not fully constrained · {n} DOF". A `status` of `redundant` maps by the same rules. When the solution has `profile_findings`, " · {outline}" follows in `warningText`, where {outline} is `outlineProblems(solution)` (2.14): "1 open end" or "{n} open ends" when every finding is `open_endpoint`, else "1 outline problem" or "{n} outline problems", n being the number of findings; the header's `data-outline` is that number (0 otherwise). An open outline, which is what actually keeps a shape from extruding, is thus named where the user looks, without changing `data-state`. When the solution has redundant constraints, " · 1 extra constraint" or " · {n} extra constraints" follows in `warningText`. During plane picking the header is a group (not a button) holding the prompt and the three plane buttons of 3.16.1, with `data-state="picking"`.
- Clicking the header opens a popover (`data-testid="sketch-problems"`) listing conflicting and extra constraints (title and a "Delete" button each) and outline problems: for each profile finding, its message from the solution (`findingMessage`, 2.14: the core's sentence, such as `The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry.`), the message's actions (such as "Connect the ends (0.3 mm apart)", run with `runFixAction`, 4.2), and a "Show" button that frames the problem: `fitBounds` in the sketch plane of the finding's points (`point`, the solved position of `connect.second`, or `start` and `end` of an overlap), grown about their center to at least a fifth of the size of the sketch's entity bounds on each axis, margin 0.1. When `dof` is above 0 it also lists "Free geometry: {counts}" (the `under_constrained` entities counted by type, for example "3 lines, 1 circle") with a button "Select them" that puts those entities in the sketch selection, so the selection inspector (3.8) offers their constraints. It always ends with `sketch_dof_help`.
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
- Used marks: during a sketch session `refreshUses` (3.9) runs when the sketch is entered and after each committed sketch edit. Every entity of the edited sketch that `usedEntities` names gets a mark (`data-testid="entity-used"`, `data-entity`, `role="img"`, a `size-4` chip with the lucide `link` icon in `textMuted` on `surface`, positioned per 3.12 at the entity's anchor as constraint glyphs are, after them on that anchor), with the tooltip and accessible name `used_by` ("Used by {labels}", 4.3, at most three labels, the users in timeline order). Marks are not pickable and follow "Show constraints".
- Deleting used entities: when a Delete removes an entity that `usedEntities` names, the sketch hint shows `entity_used` (4.3) for 8 s or until the next step prompt, for the first such entity in stored order and its first user: `'{user}' uses the {what} made from this {line, circle or arc}. Draw its replacement in the same place and '{user}' will follow it.`, or for a point `'{user}' uses the {what} made from this point.`, with `{what}` the noun of the first use of that entity by that user as 3.9 names it, without its article ("edge", "face"). A line redrawn on the same line, or a run of new lines where the old one was, keeps what used it (sketch.md section 3.14, successions); nothing is refused or confirmed, and the losses that remain raise their notification when the session ends (4.2).

#### 3.16.11 Finishing

- Finish sketch (toolbar, `Mod+Enter`), the Model or Print radio, or a feature command ends sketch mode: sketch commits still queued land first (a segment clicked just before is kept, in this sketch), then the context becomes Model (or the chosen one), the sketch session clears, and the sketch stays visible for the session; what finishing selects and reports comes from the sketch as drawn.
- Camera: when `cameraAfterSketch(camera, cameraBefore, placement normal)` (2.11) returns a camera, the view animates to it (250 ms, 2.11); otherwise the camera stays. So a camera still looking straight at the sketch returns to the view from before the sketch, or turns to `iso` when that view was straight on too, and the next feature's preview is seen in depth; a view the user already turned away during the session stays where it is.
- Finishing with Finish sketch (toolbar or `Mod+Enter`) or the Model radio sets the app selection from the finished sketch's solution: its region when it has exactly one; otherwise, when it has no region, its non-construction point entities; otherwise, when it has entities, the sketch feature itself. The selection inspector (3.8) then shows the next step (Extrude and Revolve for a region or a sketch with regions, Hole for points). A ring with a hole is two regions, so the sketch feature is selected for it. A feature command run from Sketch uses the preselection rule of 3.3 instead.
- When the solution has profile findings, the warning notification `profile_problems` offers "Back to sketch".

### 3.17 Print context

- Printer: the toolbar menu button "Printer: {profile name}", or "Printer: {default name} (default)" when the document has no printer, the default being the profile `generic-fdm` from `listPrinterProfiles`, which the engine then uses (print.md), so the user sees which printer the checks used before trusting "Ready to print". It opens `data-testid="printer-menu"` (`role="menu"`): radio items for every profile, each with a `textMuted` detail of its key numbers (`printerDetail` in `src/print/printer.ts`; FDM: "{width} × {depth} × {height} mm bed · {nozzle} mm nozzle", or "⌀{diameter} × {height} mm bed · {nozzle} mm nozzle" for a round bed; resin: "{width} × {depth} × {height} mm"), under the headings "FDM" and "Resin" by technology, then "Default ({default name})", which clears the printer. Choosing one commits `set_printer` (label "Change printer"). The status bar printer button opens the same menu. `printerButtonText` gives the button's text.
- Overhang map: a toggle (`print.overhangs`, title "Overhang map", `aria-pressed`, default on, session state) that alone shows or hides the overhang color map and its legend; switching the Overhangs check off in Checks hides its findings only.
- Checks: a popover (`data-testid="checks-popover"`) with one switch per check in this order and wording: Overhangs (`overhangs`), Bed fit (`bed_fit`), First layer contact (`first_layer_contact`), Thin walls (`thin_walls`), Small features (`small_features`), Islands (`islands`), Trapped volumes (`trapped_volumes`), Mesh validity (`mesh_validity`, always on and disabled, help "Always checked before export"). The core runs every check (protocol.md `analyze_print`); a switched-off check only hides its findings from the badge counts and the list. The choice persists in `settings.hiddenChecks`.
- Print check: the toolbar button "Print check" (with the shown finding count) opens the side panel `health`, titled "Print check". The verb "Check print" names the action of running the checks everywhere (the `none` badge, `print.check_now`, the export popover); the noun "Print check" names the panel and the badge.
- Lay on face: a toolbar button "Lay on face" (`arrow-down-to-line` icon, `print.lay_on_face`, `aria-pressed` while an orientation is set; its description, on the tooltip's second line (3.24): "Pick the face that should rest on the bed. Click again to print as modeled."; a click while the pick waits ends the pick). With no orientation set it starts a face pick (`startLayOnFace`, `print.pickingFace` true): the print hint (`data-testid="print-hint"`, `role="status"`, a chip like the sketch hint in the top-center slot of 3.2, `src/print/PrintHint.tsx`) reads `print_pick_face`, only faces are picked and only flat faces of visible part bodies are accepted (`layOnFace`; another face shows `pick_flat_face` in the hint for 3 s as a `pickHint` with no field, and the pick goes on; the hint's live region stays mounted, empty and `sr-only`, while no pick waits, so the prompt is announced when one starts), clicks select nothing, and Esc cancels (Esc ladder step 5). Leaving Print or starting a feature command ends the pick. The picked face sets `print.orientation` to `{kind: 'face_down', body, face: <its Reference>}`. With an orientation set, the button resets it to as modeled. A `CoreError` with code `invalid_orientation` or `reference_unresolved` resets it and shows the core message as a warning notification. Rows of the `overhangs`, `first_layer_contact` and `islands` checks have a button "Lay on a face" at the row's end that starts the pick (switching to Print first when the panel shows in Model); in these rows the finding text is the button that focuses the finding (`health-item`), so buttons never nest. The orientation is session state like body locks (open issue 3): it never changes the design, and while it is set the print check panel header and the export popover say so (`orientation_set`, 4.3, with "Print as modeled").

Analysis runs:

- In Print context, 300 ms after the document revision or `print.orientation` last changed with no transaction open, and on entering Print when the report is stale: `analyzePrint(documentHandle, { orientation: print.orientation ?? { kind: 'as_modeled' } })` through a `LatestOnly` (a `null` superseded result is ignored); when the answer is not `null`, `reportOrientation` records the orientation sent (null as modeled). An analysis of the same revision and orientation already in flight or waiting is not sent again. `print.running` is true meanwhile.
- Outside Print, analysis never runs on its own. The badge shows the last report as stale; clicking the badge runs the analysis and opens the print check panel.

Health badge (`data-testid="health-badge"`, a button in the status bar, `data-state`, accessible name as listed):

| State | When | Icon and text | Accessible name |
| --- | --- | --- | --- |
| `none` | No report yet | `circle-dashed`, "Check print" | "Print check: Check print" |
| `running` | Analysis in flight | `loader-circle` (`animate-spin motion-reduce:animate-none`), "Checking" | "Print check: Checking" |
| `ok` | No shown errors or warnings | `circle-check` in `successMark`, "Ready to print" | "Print check: Ready to print" |
| `warning` | Shown warnings, no shown errors | `triangle-alert` hazard sign (`warningMark`), "1 warning" or "{n} warnings" | "Print check: {visible text}", for example "Print check: 2 warnings" |
| `error` | Shown errors | `circle-x` in `errorMark`, "1 error" or "{n} errors" | "Print check: {visible text}, {w} warning(s)" when there are warnings too, for example "Print check: 1 error, 2 warnings" |

The badge counts the errors and warnings of non-hidden checks over every `report.bodies[i].report.checks[j]`, adding each check's `omitted` to the count of its listed findings' severity; info findings never change the badge. Each accessible name starts with the visible text (WCAG 2.5.3). While the analysis runs the badge shows `running` and is not marked out of date. A report whose revision differs from the document revision, or whose `reportOrientation` differs from `print.orientation`, is stale: the badge keeps its state, draws its text in `textMuted` (which keeps 4.5:1 on `surface` and `hover`, 2.16.6) and only its icon at `opacity-50`, sets `data-stale="true"`, appends " (out of date)" to its text and ", out of date" to its name. Clicking the badge in any state runs the analysis when stale or absent, and opens the print check panel.

Print check panel (side panel `health`, `data-testid="health-list"`):

- Header: "Print check: {profile name}" and a "Check again" button; while `print.orientation` is set, a line with `orientation_set` and its "Print as modeled" button under it.
- Rows (`data-testid="health-item"`, `data-check`, `data-severity`): rows come from `report.bodies[i].report.checks[j].findings[k]`; the body name comes from `bodies[i].body`, `data-check` from `checks[j].check`, and `data-severity` from the finding (`info`, `warning`, `error`; info rows use the `info` icon in `textMuted`, warning rows the `warningMark` hazard sign, error rows `circle-x` in `errorMark`). Each row shows the severity icon, the finding's message verbatim, then, when the finding has a suggestion with words, those words on a line of their own in `text` (`data-testid="health-suggestion"`), and the body name in `textMuted`. The words (`suggestionText(suggestion)` in `src/print/suggestion.ts`, null for a split without planes, which then has no line) are print.md 3.12's, in millimeters like the message above them and the printer's numbers (`printerDetail`), whatever the document's unit: each coordinate is `formatNumber` of the length in `mm` (2.13), and a tilted plane's normal components are rounded to 3 decimals (`roundToDecimals`). So the line reads as the CLI's and Python's do ("Split at x = 150 mm", "Add a drain hole near (5, 5, 5 mm)"), except that a coordinate exactly halfway between two thousandths may round the other way (`formatNumber` rounds the shortest decimal text half away from zero, `len` the binary value half to even). `HealthRow` gains `suggestion: string | null`, the finding's `suggestionText` (null without a suggestion); since the words do not depend on the document's unit, `healthRows` keeps its inputs. A row's accessible name is "{severity name}: {message} {body name}", and with a suggestion "{severity name}: {message} {suggestion text}. {body name}" (for example "Error: The part footprint is 400 x 400 mm, larger than the 256 x 256 mm bed. Split the part or scale it down. Split at x = 0 mm and y = 0 mm. Slab"), where the severity names are "Error", "Warning" and "Info". Sorted by severity (errors first), then by the check order above, then report order. Hidden checks are left out.
- Clicking a row or pressing Enter on it focuses the finding (`focusFinding` with its `FindingKey`). The finding's point (`findingPoint` in `src/print/health.ts`) is `suggestion.at` for a `drain_hole` suggestion, else `location.at` (a point location) or the center of `location.min` to `location.max` (a bounds location). A finding with a `split` suggestion that has planes and a bounds location frames that box, which holds all of its planes (`zoomToBounds`, margin 0.3; bed fit findings have the body's model-frame box, print.md 3.9.2); any other finding frames a cube of side `max(20 mm, 0.25 × the body's bounding-box diagonal)` centered at the finding's point (`zoomToBounds`, margin 0.3). A ring of 16 CSS pixels diameter stroked in `errorMark` (`data-testid="finding-marker"`, an SVG `circle` in the overlay, 3.12) is drawn at the finding's point, and a `split` suggestion's planes are drawn as "Split planes" below says. Print findings always have a location. The marker and the planes stay until another row is chosen, the panel closes, the document changes, the print orientation changes (Lay on face or Print as modeled, after which the same key can name another finding or none), a new report replaces the one the finding was taken from (a stale report's rows stay listed and can be chosen until the next analysis answers, and in the new report the same key can name another finding or none), or the finding's check is hidden (which leaves no row to explain them).
- Empty: "No print issues found with {profile name}." With no visible part body that has geometry (engine.md 3.12 analyzes only those): "Add a body to check it for printing." Before the first report: "Not checked yet."; while the first analysis runs: "Checking the model for {profile name}."

Overhang map and legend:

- Shown in Print context while Overhang map is on. For each `BodyPrint` whose `mesh_id` equals the body's current model-layer mesh id, its overhang buffer is set as the body's scalar field (3.12 pass 3); a body whose mesh changed since the report shows no map until the next report.
- The limit `a` is the `limit` of the first finding of an `overhangs` check in the report when present, else the profile's `max_overhang_angle` (print.md).
- Legend (`data-testid="overhang-legend"`, a card at the bottom of the free area's left column, under the viewport controls or, in a short free area, right of them, 3.14): title "Overhangs", a `h-2` bar with the `analysis` gradient (2.16.3; `analysisLow` to `analysisHigh`), labels "{a}°" and "90°" (degrees with `formatNumber`), and the caption "Faces steeper than {a}° from vertical need support."

Bed outline: in Print context, after a report, the bed outline of the report's profile (the `printers` entry with id `report.profile`): the rectangle [0, width] × [0, depth], or the circle of `diameter` around the bed origin, at z = 0 in bed coordinates, mapped into model coordinates by the inverse of the first `BodyPrint.transform`, drawn 1.5 CSS pixels wide in `bed`, depth-tested.

Split planes: while the focused finding has a `split` suggestion and a bounds location, each of its planes is drawn as the polygon where it cuts that box (`focusedSplitOutlines` over `splitPlaneOutline` in `src/print/suggestion.ts`). The polygon's corners are the points of the box's 12 edges that lie on the plane within 1e-9 mm (both ends of an edge that lies in the plane, else the point where an edge crosses it), each once (points within 1e-9 mm of each other are one), in order around their centroid in the plane; a plane with fewer than 3 such points draws nothing. Each polygon is uploaded as `fill` geometry whose positions are its corners in order, triangulated as a fan from the first corner, and as `lines` geometry with one segment from each corner to the next, the last back to the first. The fill is drawn at 15 percent and the outline at 60 percent opacity, 1.5 CSS pixels wide (times the UI scale), both in `problem`, in the overlay pass (3.12 pass 7, no depth test, so the cut shows through the body); neither is pickable or drawn into the ID pass, and their geometry is removed once the planes go. The planes show in whichever context the print check panel is open in, as the marker does, and the viewport mirrors the number of drawn polygons in `data-split-planes` (0 when none). The row's suggestion line names every plane in words, so the drawing is never the only sign of a suggestion.

One click that applies a suggestion (splitting the body into new bodies, or adding a drain hole) is not in M1 (1.2, ADR 0040).

### 3.18 Export

- While a feature command is active, the export commands are disabled with `finish_command_first` (export uses the model state, which ignores the focus).
- The top-bar Export button, `Mod+Shift+E` and the Document menu open the export popover (`data-testid="export-popover"`): Format (radio group 3MF, STL, OBJ, STEP; default 3MF), Bodies (radio group "All visible bodies", "Selected bodies"; the second is disabled unless bodies are selected), and an "Export" button.
- The Print toolbar's "Export 3MF" exports 3MF of all visible bodies directly; its Export group exports STL, OBJ or STEP of all visible bodies directly.
- The export popover shows, above its Export button, up to three status lines (`data-testid="export-status"`, `text-xs`; `exportStatus` in `src/export/status.ts`), so what is about to be printed is clear while export stays one click with no confirmation: (1) when an active feature has status `error`, `export_failed_feature` (one feature) or `export_failed_features` (several) with the `warningMark` hazard icon and a button "Show problems" (opens the Problems panel and closes the popover); (2) for 3MF and STL only, "Print check: {state}" with the health badge's icon and text (3.17; "Not checked" for `none`, " (out of date)" appended when stale) and, when the badge is `none` or stale, a button "Check print" that runs the analysis and opens the print check panel (`openPrintHealth`) and closes the popover; (3) for 3MF and STL while `print.orientation` is set, `orientation_set` with its "Print as modeled" button (the `arrow-down-to-line` icon in `textMuted`; the popover stays open).

```ts
// src/export/status.ts
export type ExportStatusLine =
  | { readonly kind: 'failed'; readonly text: string }
  | { readonly kind: 'print_check'; readonly text: string; readonly badge: HealthBadge; readonly checkOffered: boolean }
  | { readonly kind: 'orientation'; readonly text: string }
/** The status lines of the export popover for a format, in display order (the three above). */
export function exportStatus(state: AppState, format: ExportRequest['format']): readonly ExportStatusLine[]
/** `export_failed_feature` or `export_failed_features` for the active features in error, or null; `exportModel` uses it for `exported_with_failures`. */
export function exportFailedText(state: AppState): string | null
```
- `exportModel(request)`: the bodies are the visible bodies with model-layer geometry (for STL, OBJ and STEP only part bodies, since modifiers are not solid geometry; engine.md 3.13), or the selected bodies. With none, the error notification "There is no visible body with geometry to export." appears and the core is not called. Otherwise `exportBodies(documentHandle, format, bodies, orientation)` with `{"kind": "3mf"}`, `{"kind": "stl", "binary": true, "per_body": false}`, `{"kind": "obj"}` or `{"kind": "step"}`; 3MF and STL pass `print.orientation` when one is set, OBJ and STEP stay as modeled.
- The save location is chosen first, with `files.chooseSaveTarget('{document name}.{3mf|stl|obj|step}', kind)`, while the click that asked for the export still lets the page open the save dialog (browsers allow it only for a few seconds after a click, and a large export can take longer); then the core exports and the single returned file is written with `files.write`. A canceled picker exports nothing; a picker that fails shows the Save message for its reason with "Save as" (which asks again and exports). A failed write shows it with "Save as", which asks again where to save the same file without exporting again. Success shows the success notification "Exported {file name}.", or, when an active feature has status `error`, the warning notification `exported_with_failures` with "Show problems" instead (this covers the Print toolbar's direct exports, which have no popover). Each warning diagnostic shows a warning notification with its message. A `CoreError` shows its message as an error notification.

### 3.19 Files

- Document name: the file name without the `.ptsm` extension (case-insensitive); a new document is "Untitled".
- Replacing a dirty document (New, Open, Open recent, a recovery Open): before closing it, the UI writes its session's recovery entry as `replaced` (`id` its `recoveryId`, text `JSON.stringify(settled recipe, null, 2)`, its file reference's handle, folder and digest). That entry takes the place of the session's autosave and is never written again, so later autosaves, saves and Discards of other documents leave it alone. After the new document is open, the UI shows the info notification "{name} had unsaved changes. They are kept for you." with the action "Reopen" (opens the entry as in the recovery flow below). Replacing a document without unsaved changes removes its session's entry, if one is left.
- New (`file.new`, Document menu): closes the current document (`closeDocument`), opens `newDocument('mm')` named "Untitled", clears the selection, command and sketch session, switches to Model and resets the camera to `DEFAULT_CAMERA`.
- Open (`Mod+O`, Document menu, palette): `files.pickToOpen()`, then `openFile`. A `FileServiceError` from the picker (the desktop picker reads the chosen file, desktop.md 3.7) shows its message as an error notification and opens nothing. Dropping a file on the window shows the overlay `data-testid="drop-overlay"` ("Drop a .ptsm file to open it") while dragging and opens the first file on drop.
- `openFile(file, folder = null)`:
  1. A name not ending in `.ptsm` (case-insensitive) shows the error "{file} is not a Partsmith model. Partsmith opens .ptsm files." and stops.
  2. The bytes decode as UTF-8 with `fatal: true`; failure shows "{file} could not be read as text." and stops.
  3. **Assets** (file-format.md 3.11, ADR 0037). Without a given folder, a web handle takes the folder of its recent entry (the entry whose handle `isSameEntry` it, 2.15) when that entry has one and `folderHolds(folder, handle)`, so a file opened again with the picker keeps the folder an earlier session chose. The names are `referencedAssetNames(text)`. When there is at least one, the file has a handle and `files.reachesFolder({ handle, folder })` (always on the desktop, with a folder on the web), `files.readAssets({ handle, folder }, names)` reads them, and those are the assets step 4 gives the core; otherwise it gets none.
  4. `openDocument(text, assets)`. A `CoreError` shows "Could not open {file}. {core message}" and the current document stays, except `asset_mismatch` when assets were given (a file under an asset's name holds other bytes, engine.md 3.14): then the text is opened again without assets, and once it is open `assets_mismatch` is shown with Copy details, whose detail is the core's message.
  5. On success: stash the old document if dirty (above), close it, adopt the new handle and name with the file reference `{ name, handle, folder, digest }` (`digest` is `fileDigest(file.bytes)` with a handle, else null) (a `fileDigest` that rejects gives null, 2.15) and, as `DocumentState.assets`, the assets the successful `openDocument` received (none after the `asset_mismatch` retry of step 4, so a core restart never sends the refused files again, 4.4), remember the file in recent files (handle and folder, or text when there is no handle), switch to Model, set the camera to `iso` and fit the visible bounds, mark `ps:open`. With `migratedFrom`, the info notification "{file} was updated from format version {n}. Saving writes the current format."
  6. Once the document is shown, a `failure` of step 3 shows `assets_not_read` with Copy details, whose detail is the failure's `{entry}: {message}`, except a `permission` failure with `entry` `assets` (the browser refused the folder's permission, for example after a reload without the user's click), which counts as not reaching the folder. When a feature of the opened document has a message with code `asset_missing` and step 3 did not reach the folder, one warning says why, with `features` those features, so it goes once none of them is in error (3.22): with a handle, `assets_folder_needed` (persistent, "Choose folder", below), whose click lets the page open the folder dialog; without one (a dropped file, a file input without the File System Access API, or a recovery or recent entry without a handle), `assets_without_folder` ("Open...") when `fileSystemAccess`, else `assets_unsupported`. A host that reached the folder adds nothing, since the import's own `asset_missing` message says what to copy. In these messages and those below, `{file}` is the file reference's name, else `{name}.ptsm` with the document name.
- Save (`Mod+S`): with a file handle, writes the document to it (below) with the change check; without one, Save as.
- Save as (`Mod+Shift+S`): `files.chooseSaveTarget('{name}.ptsm', 'ptsm')` first (a cancel stops with nothing changed; a `FileServiceError` from the picker, such as the desktop dialog refusing a name, desktop.md 3.7, shows the Save message for its reason with `{file}` the suggested name and the action "Save as", and saves nothing), then writes the document to the target (below) without the change check, since the dialog confirmed replacing the file. A handle target becomes the document's file once it is written (the name from the handle, the folder of step 3 and the digest of step 5) and is remembered in recent files with that folder; a download target clears the handle, and the first download in a session shows the info notification "This browser saves by downloading. {file name} is in your downloads folder."
- Writing the document (Save, Save as and their "Choose folder"), in the order of file-format.md 3.11 (ADR 0036, 0037). Writes run one at a time per app: one that starts while another is in flight waits until that one has ended, and is then built from the document as it is: Save takes the document's file (handle and folder) then, so it goes to the file a Save as before it chose, and a write whose document session was replaced meanwhile writes nothing. Each step that stops sends no `markSaved`, so the core stays dirty and the document unsaved:
  1. `saveDocument(documentHandle)`: the text, the assets and the revision.
  2. **Change check** (Save, when the file reference's `digest`, read as the check runs, is not null): `files.readHandle(handle, 'readwrite', folder)` with the document's folder on the web, so the prompt this click may show is the folder's, which in Chromium also covers this read and the writes after it (2.15, open issue 39), and `fileDigest` of its bytes. Another digest shows `save_conflict` and stops before anything is written; a `FileServiceError` shows the Save message of its reason (step 4) and stops. A file rewritten elsewhere with the same bytes counts as unchanged, and a `fileDigest` that rejects skips the check (2.15).
  3. **Assets**, when the result has any. The place is the target's handle with, on the web, the document's folder for Save, and for Save as the document's folder when it holds the new file (`folderHolds`), else none. When `files.reachesFolder(place)`, `files.writeAssets(place, assets)`; a `FileServiceError` shows `save_assets` with "Save as" and Copy details (the failure's `{entry}: {message}`) and stops. When it does not reach the folder (on the web, Save as into another folder), the persistent warning `save_folder_needed` offers "Choose folder" and the save stops with nothing written by the page (the save dialog itself may have created or emptied the target, open issue 37). Its action runs `files.chooseFolder(target handle)`: `chosen` writes the document again from step 1 to the same target with that folder, `other` shows `assets_wrong_folder`, whose "Choose folder" asks again for the same save, and `canceled` shows `save_folder_needed` again for the same target, since running an action dismissed it (3.22). A download target never gets here: without the File System Access API the page reads no folder, so the core holds no assets.
  4. **Document**: `files.write(target, 'ptsm', utf8)`. A `FileServiceError` shows: permission: "Could not save {file} because Partsmith no longer has permission to write it." (action "Save as"); not_found: "{file} was moved or deleted. Save it again to choose a location." (action "Save as"); io: "Could not save {file}: {reason}" (action "Save as").
  5. The file reference's `digest` becomes `fileDigest` of the written bytes (null when it rejects), even when step 6 fails, since the file now holds them.
  6. `markSaved(documentHandle, revision)` with the revision of step 1, then `localUnsaved` false and, when the document has no unsaved changes left, the document session's recovery entry removed (every other entry stays); an edit made while the file was written leaves the document dirty, so its entry and pending write stay.
- `save_conflict` (step 2 found the file changed elsewhere, error): "Overwrite {file}" sets the file reference's digest to the one step 2 read and saves again, so a file changed once more warns again; "Save as" runs Save as; "Open the newer file" reads the file again (`readHandle`) and opens it with `openFile` and the document's folder, so the unsaved document is kept as a `replaced` entry with `stashed` and "Reopen" (above). A `FileServiceError` of that read shows `open_failed` with its message.
- Choose folder (the action of `assets_folder_needed`, and of an `assets_wrong_folder` that follows it) runs `files.chooseFolder(handle)`: `canceled` shows `assets_folder_needed` again with the same `features`, since running an action dismissed it (3.22), and `other` shows `assets_wrong_folder` (error, "Choose folder"). A `FileServiceError` of `chooseFolder` (here and for `save_folder_needed`) shows its message as an error notification, as a failed Open picker does. `chosen` keeps the folder in the document's file reference, remembers it in recent files, and reads `referencedAssetNames(JSON.stringify(settled recipe))` from it (`readAssets`; a failure shows `assets_not_read`). When it read no asset, nothing else happens: no request is sent, and the imports' own `asset_missing` messages say what to copy. When it read at least one, it reopens the document with them as a core restart does (4.4), without stopping the core, in one `other` task of the mutation queue. When that task's turn comes, after every edit queued before it, it does nothing unless the shown document is still the one the action was offered for, and a feature command or transaction opened meanwhile refuses it with `finish_command_first` and shows `assets_folder_needed` again with the same `features`; otherwise it takes the settled recipe as it is then, runs `openDocument(JSON.stringify(settled recipe, null, 2), assets)`, closes the old document (`closeDocument`), leaves sketch mode and shows the reopened document. An edit made while it runs waits for it and goes to the reopened document. The session keeps its name, file reference, `recoveryId` and recovery entry, gets those assets, and has `localUnsaved` true when it was unsaved before, else false; the selection is cleared and undo history starts again. `asset_mismatch` leaves the shown document as it is and shows `assets_mismatch`.
- **Actions offered for one document.** The actions of `save_conflict`, `save_folder_needed`, `assets_folder_needed` and `assets_wrong_folder` each do nothing (no file service call and no request) unless the shown document is the session they were offered for (its `recoveryId`) with the file reference handle it had then, so a click after New, Open, a recovery Open or a Save as to another file never saves one document into another one's file or reopens the wrong document. Their "Choose folder" and "Overwrite {file}" have `editsDocument` true: during a feature command they wait, and running one refuses with `finish_command_first` (2.9, 3.7).
- Open recent (Document menu submenu, palette items): up to 10 entries, newest first, each showing the name and its open time (`toLocaleString('en-US', { dateStyle: 'medium', timeStyle: 'short' })`). An entry with a handle reads it (`readHandle(handle, 'read', folder)` with the entry's folder, so the one prompt this click may show is the folder's, 2.15) and opens it with `openFile` and the entry's folder: permission denied shows "Partsmith needs permission to open {file}. Choose it again to grant access." (action "Open..."); not found shows "{file} has moved or been deleted, so it was removed from recent files." and forgets it. An entry with text opens that text (no handle; Save goes to Save as).
- Recovery copy: each document session, from the New, Open or recovery Open that adopts a document until that document is replaced, keeps one entry under its `recoveryId`; a core restart continues the session (4.4), even one that a New or Open ended before the stopped core dropped it, and writes its entry at once (`reason: 'autosave'`), so a write that fell due while the core restarted is not lost. The entry holds the settled recipe (1.3): a `document_changed` with `transactionOpen` true neither writes nor delays it, so the edits of an open feature command, including a preview that stops the core, never come back as committed features. While the document is unsaved (`dirty` or `localUnsaved`), the UI writes the entry (`reason: 'autosave'`, `JSON.stringify(settled recipe, null, 2)`, the file reference's handle, folder and digest) 2 s after the last `document_changed` with `transactionOpen` false, and at most 10 s after the first such change that no write has stored yet, so a stream of changes is still kept. When a write is due, it happens only if the session's document is shown and unsaved, and a replaced document's pending write is dropped, so a stash is never overwritten. A `document_changed` with `transactionOpen` false that leaves the document without unsaved changes (after a save, or an undo back to the saved revision) removes the entry and drops its pending write; a successful save that leaves no unsaved changes and Discard also remove an entry, each only its own. `beforeunload` calls `preventDefault()` while the document is dirty so the browser asks before leaving.
- At startup the UI offers the listed entries (2.15) one at a time, newest first, each with a persistent info notification: `recovered` ("Partsmith kept unsaved changes to {name} from {time}.") for the last one, or `recovered_more` ("Partsmith kept unsaved changes to {name} from {time}, and to {n} other documents.") while {n} more wait. Its actions are "Open" (opens the text as a document named {name} with the stored handle, folder and digest as its file reference, reading its assets and saying why any are missing as `openFile` steps 3, 4 and 6 do, and sets `localUnsaved` true; "Reopen" of `stashed` opens its entry the same way) and "Discard" (removes that entry only); once either has finished, the next entry is offered. Dismissing the notification offers no more, and the remaining entries stay for the next startup.
- Open and Reopen start a new document session: once the core has opened the text, the UI writes it at once under the new `recoveryId` (`reason: 'autosave'`, the opened entry's `savedAt`, handle, folder and digest) and, once that write is stored, removes the opened entry, so the copy survives a crash right after opening, and two tabs that open the same entry each keep their own. When the write is not stored (IndexedDB full or blocked), when the core refuses the text (`open_failed`) and when the core stops before the text is open (`recovery_open_stopped`, 4.4), the opened entry stays.
- The desktop host replaces `FileService` with native dialogs (open issue 2) and reaches the `assets/` folder next to a granted file through its grant (desktop.md 3.4), so it never needs a folder chosen; every behavior above stays.

### 3.20 Undo and redo

- `edit.undo` and `edit.redo` call the core's `undo` and `redo` through the mutation queue. Their palette details are "Undo {undoLabel}" and "Redo {redoLabel}"; with nothing to undo or redo they are disabled with "Nothing to undo." or "Nothing to redo." After either, a polite live region announces "Undid {label}" or "Redid {label}".
- With a feature command active, `Mod+Z` and `Mod+Shift+Z` step through the command history (3.7). With an empty history `Mod+Z` cancels the command, and with an empty redo stack `Mod+Shift+Z` is disabled with `finish_command_first`. Their palette details are "Undo change to {field label}", "Redo change to {field label}" and, with an empty history, "Cancel {title}".
- In sketch mode with an operation in progress (rubber band, drag, box selection, open editor), `Mod+Z` cancels that operation only.
- Sketch view history (technical design 13, principle 6): during a sketch session the UI keeps a stack of entries, each either `view` (a camera) or `document` (a marker for a committed sketch edit). Entering the sketch pushes the camera from before entering; each completed orbit or pan gesture, each wheel burst (ended by 300 ms without wheel events), view cube click, fit, home and look at pushes the camera from before it; each committed sketch edit pushes a `document` marker. An `apply` action run during the session (a finding's "Connect the ends", a fix from the Problems panel, 4.2) pushes a `document` marker like a committed sketch edit. In sketch mode, `Mod+Z` pops the top entry: a `view` entry restores its camera (animated) and moves to a redo stack with the camera it replaced; a `document` entry calls the core's `undo`. With the stack empty, `Mod+Z` calls the core's `undo` directly. `Mod+Shift+Z` mirrors this with the redo stack. A new view change or edit clears the redo stack. Leaving sketch mode discards both stacks.

### 3.21 Status bar

Left to right (`data-testid` in parentheses):

- Core status (`status-core`), shown only when not idle: "Loading core" with a spinner, "Rebuilding" with a spinner when a mutating request has been pending for more than 300 ms, "Still working" with a spinner in its place while `core.stillWorking` is true (4.4; background requests such as a print check never make it true on their own), or "Core stopped" in `errorText`. Spinners are `loader-circle` with `animate-spin motion-reduce:animate-none`.
- Sketch DOF (`status-dof`, sketch mode with a sketch only): "{n} DOF" (with `tabular-nums`, as every status bar item that shows a number, 2.16.7), with `data-dof` and `data-state` as the sketch header, and the tooltip "Degrees of freedom: how many ways the sketch can still move."
- Grid (`status-grid`, shown while a grid is drawn, 3.12): "Grid {formatQuantity(spacing)}" ("Grid 10 mm") in `textMuted`.
- Rebuild time (`status-recompute`): "Rebuilt in {n} ms" (integer) or "Rebuilt in {s} s" (one decimal) for durations of 1000 ms or more, measured from sending the last mutating request to receiving its response; tooltip "Time the last change took to rebuild".
- Units (`status-units`): a button with the unit ("mm") opening a menu of radio items mm, cm, m, in, followed, while `document.bareNumbers` is above 0, by the checkbox item `keep_sizes` (`data-testid="keep-sizes"`, `menuitemcheckbox`): "Keep sizes: add {unit} to the {n} numbers without a unit" ("the 1 number" for one), with `{unit}` the current unit and `{n}` `bareNumbers`, checked while `keepSizes` is on (on by default; toggling it keeps the menu open and lasts for the session). Choosing a unit other than the current one runs `setUnits`: `set_document_units` with `keep_sizes` true when the item is shown and checked, else false (engine.md section 3.25), one undo entry labeled "Change units to {unit}", and shows the info notification "Units are now {unit}. Numbers typed without a unit now mean {unit}." (model.md 3.12). With Keep sizes the numbers without a unit get the old unit (`20` becomes `20 mm`), so no size changes. The palette's `document.units.*` commands do the same with the current `keepSizes`.
- Printer (`status-printer`): a button with the profile name, or "{default name} (default)" when the document has none, opening the printer menu (3.17).
- Print check badge (`health-badge`, 3.17).

### 3.22 Notifications

- Stack at the free area's bottom-right: the region after the status bar (`relative z-30`, above the overlay islands, 3.2), filling the free area's grid cell with `p-4`, `pointer-events-none` and its cards aligned to the bottom-right, each card `w-90 max-w-full`, newest at the bottom, at most 3 shown; each is a card (3.2): `surfaceRaised` with a `forcedEdge` border, `rounded-xl` and `shadow-lg` with `elevation`, since it can float over an island or the inspector. A fourth removes the oldest non-error notification (or the oldest when all are errors).
- Each (`data-testid="notification"`, `data-severity`, and `data-code` when it has a `code`) has a severity icon (`info` in `textMuted`, `circle-check` in `successMark`, the `triangle-alert` hazard sign of `warningMark`, `circle-x` in `errorMark`), the message (left-aligned, `text`), action buttons, and a "Dismiss" button. Running an action dismisses the notification.
- Info and success dismiss after 5 s, warnings after 8 s; errors stay until dismissed. Timers pause while the pointer is over the notification or focus is inside it.
- Errors are announced through a `role="alert"` region; the others through a `role="status"` region.
- A notification with the same message as a visible one does not appear again; the visible one takes the new one's actions, code and named features (they belong to the latest occurrence, such as the command a Restore reopens) and restarts its timer.
- A failure notification that names features (`Notification.features`) is dismissed automatically when none of them is in error anymore (after an undo or a fix), so it never outlives its cause (4.2). A loss notification is dismissed automatically when, besides, none of its `problems` remains.
- A notification with a `cause` shows it under the message, in `textMuted` (`data-testid="cause-line"`), above the action buttons.
- The loss and failure notifications of a user change are computed by `changeNotifications` (4.2) from the statuses before and after the change:

```ts
// src/problems/problems.ts
/** A notification a change raises (4.2), before its actions are bound. `problems` are loss keys `{feature} {code} {field}`. */
export type ChangeNotice =
  | { readonly kind: 'lost'; readonly root: string; readonly description: string; readonly failed: number; readonly severity: 'warning' | 'error'; readonly cause: Cause | null; readonly features: readonly string[]; readonly problems: readonly string[] }
  | { readonly kind: 'check'; readonly root: string; readonly cause: Cause | null; readonly features: readonly string[]; readonly problems: readonly string[] }
  | { readonly kind: 'failed'; readonly features: readonly string[] }
/** The notices of one change, losses in timeline order of their roots, then at most one `failed` notice. */
export function changeNotifications(before: Readonly<Record<string, FeatureStatus>>, after: Readonly<Record<string, FeatureStatus>>, document: Document): readonly ChangeNotice[]
```

### 3.23 Visual design

- Direction: a precision instrument used for hours at a time ([docs/guidelines/README.md](../guidelines/README.md)). Graphite and zinc surfaces, generous spacing and restrained motion; the viewport fills the window and the model is the hero, while the chrome floats over it as flat islands that recede through quiet color, size and weight (3.2): a step lighter than the viewport in both themes, with no outline and one faint shadow, never as panels docked behind hairline borders. It avoids the frontend guideline's generic looks: no decorative gradients (the only gradient is the analysis legend; the viewport is one flat tone, 2.16.4), no cream or off-white backgrounds (surfaces are neutral zinc), no pill-shaped buttons, no monospace labels, no numbered section labels and no italic accent words.
- Color: by role only (2.16.3, 2.16.4). One signal orange for selection, focus, the current tool and the primary action; blue only for under-constrained sketch geometry; red only for errors and conflicts; yellow hazard signs for warnings; green checks for success; the sky-to-violet ramp for analysis; rose, green (emerald light, lime dark) and sky for the X, Y and Z axes, always beside their letter except on the origin planes and the view cube's edges (2.16.3). Everything else is zinc.
- Type: three roles (2.16.7). Interface, Geist with its serifed capital I (`ss05`) set once at the root, for all interface text, units and numbers. Numbers, which add `tabular-nums` wherever digits line up (fields, readouts, measurements, the Parameters panel's values, dimension labels, the timeline's and the status bar's counts and timings) and stay proportional in sentences, in counts within prose and in button labels. Code, JetBrains Mono through `font-mono`, for parameter names and expressions shown as code only; an inspector field stays Interface even while it holds an expression. Body text `text-sm` (0.875rem), secondary text and captions `text-xs` (0.75rem, the minimum anywhere), panel titles `text-base font-semibold`; titles are the interface face at semibold, and there is no display face. Axis letters are `font-semibold` in the interface face, at the size of the text they label (`text-xs` inside fields). Island and type rules name the roles, never a family (2.16.7).
- Alignment: left by default (text, rows, panel headers, field labels, notifications, empty states). Centered only where this spec says so: the palette's position, the top-center slot of the viewport (the empty-state card, the sketch header, the sketch hint and the print hint, 3.2), the loading and fatal overlays and the view cube's face labels.
- Spacing on Tailwind's 0.25rem scale: a 0.5rem gutter around and between islands (`p-2` on the app root and each island's `mb-2`, `mr-2`, `ml-2` or `mt-2` toward the free area, 3.2), panel padding `p-4`, panel sections separated by `gap-4` alone (3.2), rows and controls `h-8`, toolbar buttons `h-9` with labels (`px-3 gap-2`) and `size-9` icon-only, gaps `gap-2`, `gap-3` and `gap-4`.
- Shape and depth, in the tiers of 3.2: `rounded-xl` (0.75rem) for islands and multi-line cards (the inspector, notifications, the viewport controls, the overhang legend, the empty-state card, the core panels and the drop overlay) and the palette; `rounded-lg` (0.5rem) for menus, popovers, tooltips, the one-line chips over the viewport, status banners and islands under 2.5rem tall; `rounded-md` (0.375rem) for controls; `rounded-sm` (0.25rem) for marks, swatches and segments; never `rounded-full` on a button. Depth is a tonal step plus one faint shadow, never an outline (3.2): islands, cards and chips on `surface`, a step lighter than the viewport, and what floats over them on `surfaceRaised`, a step lighter again in the dark theme; Tailwind's `shadow-md` for frame islands and the cards and chips that stay (viewport controls, legend, empty-state card, the sketch header and hint, the print hint, the view cube's Home view button), `shadow-lg` for what comes and goes over them (the inspector, notifications, core panels, overlay islands, the palette, menus, popovers, tooltips, the dimension editor, the timeline's drop hint, the drop overlay), both colored by the `elevation` role; `shadow-xs` only under a segmented control's thumb; `shadow-sm` is not used. Islands, cards, popups and chips keep a transparent 1-pixel `border` in `forcedEdge` for forced colors (3.24). Borders that carry meaning stay: control boundaries in `borderControl`, the `accent` borders of the active selection field, of primary buttons, of the timeline's drag ghost and of the drop overlay (dashed), and the invalid and warning field borders. Lines inside islands are `divider` hairlines only where groups cannot be told apart otherwise (3.2).
- Buttons: the primary action is `accent` filled (2.16.3), with a border in its fill's color (`accent`'s border facet), the toolbar's primary buttons (Finish sketch, Export 3MF) included, so forced colors draw its edge; a disabled primary button keeps its border in `forcedEdge`, so its size, and the toolbar's fit, never change. Every other `Button` is tonal, `tonal` with the `hover` fill on hover and `text`, so it reads as a button without an outline; a disabled one keeps `tonal`, in `textDisabled` and without the hover. On a status banner (`inspector-error`), whose `errorSubtle` and `warningSubtle` fills the `tonal` steps nearly match (1.00 to 1.08, 2.16.6), a `Button` takes `variant="banner"`: the `banner` facets of `tonal` and `hover` in place of their `background` facets, translucent `zinc-950` in the light theme and white in the dark theme, so the one-click fixes and repair candidates keep their button shape on either banner. A `Button` given `aria-pressed` is a toggle (the dimension editor's Reference, 3.16.7): while pressed it takes `selection` and `text` in place of `tonal` and `hover`, with no hover, like the other pressed toggles, and the state outline below. Icon buttons, toolbar buttons and the top bar's Search and Export are ghost buttons: no fill at rest and the `hover` fill on hover (Search in `textMuted`, Export in `text`). The default `Button` keeps a 1-pixel `border` in `forcedEdge`, which forced colors draw (3.24); ghost buttons, Search and Export among them, have no border, as they have no fill to lose.
- Segmented controls (the context switcher, choice fields, the section plane, the navigation preset), in the manner of shadcn/ui's Tabs: the track (`role="radiogroup"`) is `h-8 rounded-md border p-0.5` in `tonal` with a `forcedEdge` border, and its segments are `rounded-sm`. The checked segment is the thumb, raised over the track: `thumb` with `shadow-xs` in `elevation`, `text` and `font-semibold`. The semibold `text` label names the chosen option, since the thumb's step is decorative (1.09:1 in the light theme, 2.16.6). Every segment label carries `forcedEdge`'s outline facet (`outline-transparent`) at rest, and only the checked one adds `outline -outline-offset-1`, so forced colors outline the chosen option (3.24) and `transition-colors`, which includes `outline-color`, never fades a gray outline out of a newly checked segment. Unchecked segments have no fill and are `textMuted`, turning `text` on hover (the `hover` role's foreground facet). A disabled control keeps its thumb, its outline and the checked label's `font-semibold`, with the checked label in `textMuted` (in `textDisabled` it would measure 1.35:1 on the dark thumb) and the others in `textDisabled`, and no hover. Each radio keeps `focusRing`, whose ring lies on the track, on a neighboring thumb or on the island or popup (`accent` on `tonal`, `thumb`, `surface` and `surfaceRaised`, 2.16.6). `selection` is not used in segmented controls.
- State outlines: every element shown selected, pressed or active by a fill adds `outline -outline-offset-1` in `forcedEdge`'s outline facet while in that state, as the checked segment does, so forced colors, which drop the fill, still mark it (3.24): selected browser rows and timeline items, the print check's focused finding, pressed toolbar, icon and viewport toggles and the active tool, a pressed toggle `Button`, the palette's active option, and the keyboard-active item of a menu, the printer menu and the `expression-suggestions` list. Pressed toggles fill with `selection`, except the top bar's Toggle browser and Dark theme, which fill with `hover`'s `active` step and `text` (`IconButton`'s `pressedFill="neutral"`): they switch what the window shows, which is neither a tool nor a selection, so the orange of `selection` keeps its meaning (2.16.3) and the top bar's corners stay quiet. These elements carry the outline facet at rest in place of the `accent` outline facet that other buttons keep at rest for their focus ring, so `transition-colors` fades no gray or orange outline in or out with the state; their focus ring then fades in over the same 150 ms. The timeline item being edited takes its `accent` `outline-2` in place of the state outline, and `focusRing`, whose variants come later in the stylesheet, draws over either.
- Icons: lucide-react line icons with `strokeWidth={1.75}` (a component prop, not a style), sized with `size-5` in toolbars and `size-4` in lists and fields, colored through `currentColor` from the element's foreground role (`text`, `textMuted`, or a status role for status icons).
- Motion, restrained and with Tailwind utilities only: menus, popovers, tooltips, the palette and the inspector fade in over 150 ms (`transition-opacity duration-150 ease-out` with `starting:opacity-0`, or `starting:open:opacity-0` for popovers, which stay in the DOM while closed); hover color changes use `transition-colors duration-150`; spinners use `animate-spin`. Nothing slides or bounces. `motion-reduce:transition-none` and `motion-reduce:animate-none` remove all of it under `prefers-reduced-motion`, and camera animations (250 ms, 2.11) become instant. There is no animation library in M1.
- Focus: `focusRing` (a 2-pixel `accent` outline offset by 2 pixels on `:focus-visible`, Tailwind's `outline-2` and `outline-offset-2`) on every interactive element. The controls of islands under 2.5rem tall (the status bar's items and its health badge, whose `overflow-x-auto` clips both axes, and the collapsed timeline's buttons) take its `inset` facet instead, the same ring drawn 2 pixels inside the control, which the island's edge cannot cut. The 3D view's application element fills the window, where its own outline would lie under the islands, so it carries the class `peer` and no outline, and the free-area layer that follows it draws the ring around the free area instead (`peer-focus-visible:outline-2 peer-focus-visible:-outline-offset-2` with `rounded-xl` and the `accent` outline facet).

### 3.24 Accessibility

- Contrast: every pair of the text and non-text tables of 2.16.6 meets WCAG 2.2 AA (text 4.5:1, non-text 3:1) in both themes; U-70 enforces them. Text drawn over the viewport always sits on a `surface`, `warningFill` or `errorFill` chip. The palette's search field is the one input without a boundary, as before ADR 0032: it is the open palette's only input, focused while the palette is open and identified by its search icon, its placeholder (`textMuted`'s placeholder facet) and its caret, and WCAG 1.4.11 asks a boundary only where the boundary is what identifies the control; the rule under it is a decorative `divider`.
- Forced colors (Windows High Contrast; `@media (forced-colors: active)`): the browser drops every `box-shadow` and paints backgrounds in the system's Canvas color, so the tonal steps and shadows of 3.2 vanish. Every island, card, popup and chip then keeps a 1-pixel edge from its `forcedEdge` border: Tailwind's `border` (1 CSS pixel, solid by the preflight) with `border-transparent`, whose transparent color the system replaces with its text color (CanvasText). The default `Button` and a disabled primary button, segmented control tracks, constraint glyphs and dimension labels, and a `Switch`'s thumb (whose `bg-current` fill turns Canvas) keep theirs the same way, enabled primary buttons their `accent` border, and the rule of each scrolling body of 3.2 shows at all times. Every state shown by a fill keeps its state outline (`outline -outline-offset-1 outline-transparent`, 3.23): the checked segment, selected rows and timeline items, the print check's focused finding, pressed toggles and the active tool, the palette's active option and the keyboard-active item of a menu or list; the checked segment is also named by its semibold label. Ghost buttons (toolbar and icon buttons, Search and Export) draw no edge, as at rest. Control boundaries (`borderControl`) and the focus ring are recolored by the system and stay. Colors that are content keep their own: the color popover's swatches and the overhang legend's ramp bar carry `forced-color-adjust-none`, since forced colors would paint the swatches in Canvas and drop the ramp's gradient while the 3D view, which forced colors do not touch, keeps its colors (a body's color button draws an SVG `fill`, which keeps its own). No other element opts out, and no `forced-colors:` variant is used. E-146 checks this with Playwright's forced-colors emulation.
- Keyboard: every function is reachable without a pointer except picking faces, edges and vertices in the viewport, drawing sketch geometry, dragging sketch points and dragging the inspector (later). Selection fields for bodies, sketches, planes, axes and regions offer the "Choose" menu.
- Tab order: top bar (document name, context radios, toolbar, palette button, Export, theme toggle), browser (one stop), viewport (one stop), the buttons of the top-center slot while shown (the empty-state card's, the sketch header or its plane buttons), the inspector when open, timeline (one stop), side panel, status bar buttons, notifications. Opening the inspector moves focus into it; closing it returns focus. During a feature command, Tab on the focused viewport goes to the command's value field instead (3.7). Focus never drops to the page when the control that had it goes away: plane picking gives it to the "Top" plane button, entering a sketch from a plane button to the 3D view, and closing a side panel from inside it back to the control that opened it (the 3D view when that control is gone; `src/layout/focus.ts`).
- Roving tab index with arrow keys in toolbars, segmented controls, the timeline, the tree and menus. Menus close on Esc and return focus to their button.
- Tooltips appear after 500 ms of hover and immediately on keyboard focus, and close on Esc. Esc never opens one: after an Esc, focus shows no tooltip until another key is pressed, so the trigger that gets focus back when an Esc closes its menu, popover or dialog shows none (the browser returns that focus while it is still closing the popover, and Chromium 156 throws on a popover shown then, which would crash the region, 4.4). Tooltips are referenced by `aria-describedby`, and show the command title and formatted shortcut, then the command's description (2.10) on a second line in `textMuted`. Field help (3.7) shows on hovering a field's label and reaches assistive technology through the control's `aria-describedby`.
- Every interactive target is at least `size-6` (1.5rem, 24 CSS pixels at 100 percent scale). Every icon-only button has an accessible name.
- Color is never the only signal: status has icons and text (warnings a hazard shape, errors a crossed circle), DOF has the header text, selection has thicker strokes, the analysis map has a legend, and an axis color always comes with its letter (fields, readouts, axis labels, the browser's axis rows), except the origin planes, named by their plane labels, and the view cube's edges, named by its face labels, so red-green color blindness loses nothing; hovered and selected axes and plane outlines are also drawn thicker (3.12).
- At 200 percent UI scale on a 1280 by 720 CSS-pixel window every region stays usable (toolbars go icon-only, primary buttons included, and the tools that do not fit are in More tools, 3.2; panels scroll; the browser and side panel float over the free area as overlays, 3.2; the view cube, the viewport controls, the overhang legend and the top-center slot keep their places and clear each other in the 1248 by 376 pixel free area, 3.14).
- Live regions: selection, undo, redo and Copy details announcements, notifications, the sketch hint and the print hint (step changes only, 3.16.3), and the timeline's drop hint (3.9).
- Repair candidates are reachable without a pointer: their buttons in the inspector banner (or the re-attach header) are ordinary buttons, and the viewport markers repeat them for the pointer only (`aria-hidden`).

## 4. Errors

### 4.1 Message rules

Every message the user sees follows the architecture's error rule:

- Plain sentences a maker understands, starting with what happened, naming the cause in the user's terms (feature labels, body names, file names, values with units), and naming the fix when one exists.
- No "Error:" prefix, no exclamation marks, no blame, no stack traces, no error codes in the visible text (codes go to `console` and `data-code` on the notification).
- At most two sentences, then the message's action buttons, then the cause line ("This started after you ...", in `textMuted`, `data-testid="cause-line"`) with its own action buttons. Action labels start with a verb and are the protocol's `FixAction` labels ("Use 2.3 mm", "Set Count of 'Linear pattern 1' back to 3"); the UI's own labels follow the same rule ("Save as", "Undo", "Edit", "Replace them"). A "Copy details" action copies the code, the message, the cause and `detail` for reports.
- Copy details is the last button of every core message or core error that has a `detail` (the inspector banner, the Problems panel, the selection inspector, and the error notification of a `CoreError`), of the UI's `save_assets`, `assets_not_read` and `assets_mismatch` (3.19), whose detail is the failure's entry and host text or the core's message, and of no loss notification (4.2). `copyDetails` writes to the clipboard (`navigator.clipboard.writeText`) the code, the message, the cause's text when there is one, and the detail, each on its own line in that order (lines joined with `\n`, no trailing newline), then announces `details_copied` in the polite live region. A clipboard that refuses (permission, insecure context) shows `details_not_copied` (4.3) as a warning notification instead.
- Core messages (`CoreError.message`, `Diagnostic.message`, finding messages) are shown verbatim; the core follows [the error guidelines](../guidelines/errors.md) (features.md section 4, protocol.md section 4). The UI only wraps them in the templates of 4.3, and never shows a `detail`, a `reason`, a score or a `FixRef` as text.
- File names are quoted as given, without quotation marks. Feature and body names appear in single quotes where the core puts them and in the UI's texts of losses, causes and uses (4.3: `feature_lost`, `features_lost`, `feature_check`, `show_lost`, `check_feature`, `needs_closed_shape`, `drop_breaks_use`, `used_by`, `entity_used`, `reattach_prompt`, `attach_label`, `attach_after`), which name features as the error guidelines do; the older UI-owned texts name them without quotes (open issue 19).

### 4.2 Where errors appear and the fixes they offer

| Error | Where | Fix actions |
| --- | --- | --- |
| Invalid expression in a field | Under the field (`field-error`, `errorText`), field border `errorMark`; OK disabled | Edit the field |
| Invalid sketch dimension value | In the dimension editor: `dimension_over_constrained` for a new dimension, the probe's message for an edited one (3.16.7) | Edit the value, "Add reference", "Cancel" |
| Feature status during a command (preview) | Inspector banner (`inspector-error`) with each message's actions and cause line, the mapped field's `field-error`, and the limit line under a fillet, chamfer or shell field (3.7) | Errors and warnings whose `field` maps to a field: the field is highlighted and focused by clicking the message. The message's and its cause's actions, run with the command rules of 3.7: an `apply` action joins the transaction and updates the command's params, and waits while the preview is pending. In a feature with an inline Reverse (2.12): an `out_of_range` error whose `field` maps to the field Reverse is attached to (`attachedTo`), holding a plain number (2.13) that starts with `-`, offers "Reverse and use {text without the leading -}", which toggles Reverse and sets that text. The UI offers no Reverse of its own for `no_effect` or `through_all_misses`: the core offers "Turn on Reverse" only when reversing reaches the body (features.md section 3.15) |
| Feature status after a user change: a feature goes from not-error to error | One notification (`Notification.features` naming those features): for one feature "{label} failed. {message}" with "Edit" and "Undo"; for several "{n} features failed after this change." with "Show problems" and "Undo". Features that a loss notification of the same change reports (next row) are left out of it, and with none left it is not shown. During a sketch session, sketch edits raise none (the timeline and browser badges show the failures); when the session ends, the features in error that were not in error when it started raise this notification once. A feature command counts as one change, compared with the statuses from before it began. The notification is dismissed automatically when none of the features it names is in error anymore | As listed |
| Feature losses after a user change: a feature gains a new lost, guessed or widened reference, a lost, ambiguous or resized region, a region matched again by its size and place (a guess, features.md section 3.7), a sketch face fallback or a turned-over sketch face (codes `reference_missing`, `reference_ambiguous`, `reference_guessed`, `reference_widened`, `region_missing`, `region_ambiguous`, `region_resized`, `region_remapped`, `plane_fallback`, `sketch_turned_over`, and the errors with repairs `references_not_found`, `reference_not_found` and `profile_not_found`), warning or error | One notification per root (`changeNotifications`, 3.22). A loss is new when its key (feature, code and field) was not in the statuses before the change. Each new loss, and each feature newly in error, belongs to a root: the feature that its first message's `root` names when that is another feature of the document, else its own feature. A root with a new loss of its own gets one notification, with `features` the root and the features of its group newly in error, `problems` its new loss keys, and `cause` the cause text of its first new loss message: when that loss has a repair and its code is none of `reference_guessed`, `reference_widened`, `region_resized`, `region_remapped` and `sketch_turned_over`, `feature_lost` ("'{label}' lost {description}.", `{description}` the `description` of the message's repair at its `field`, else of its first repair), or `features_lost` ("'{label}' lost {description}, so {n} features after it failed too.") when n features of its group, 1 or more, are newly in error; severity error when a feature it names is in error, else warning. Otherwise `feature_check` ("'{label}' needs a check after this change."), a warning. During a sketch session, sketch edits raise none; when the session ends, the new losses since it started raise these notifications once. A feature command counts as one change, as above. Info notices never notify. The notification is dismissed automatically as 3.22 states | `feature_lost`, `features_lost`: `show_lost` ("Show what '{label}' lost", `showLost`: repair mode, framed), the first `apply` action of the cause line when it has one, and "Undo". `feature_check`: `check_feature` ("Check '{label}'": repair mode when its status has repairs, else `editFeature`), the cause's first `apply` action, and "Undo". Never a candidate: a replacement cannot be judged without seeing it |
| Feature warnings and errors at rest | Timeline and browser badges, tooltips, Problems panel (grouped by root, 3.9), selection inspector | Every `FixAction` of the message, then of its cause, in order (`runFixAction`): `apply` runs `apply_fix` with its `FixRef`, one undo entry labeled with the action's label, and a `stale_fix` error shows the core message as a notification with "Show problems"; `pick` runs `editFeature` of its feature with the field its `path` maps to active (3.7), or `reattachSketch` for a sketch's plane (3.16.1), in place of the former "Re-pick {field label}"; `show` selects and frames the feature and highlights `entity` with the cause's ghost (Show, below); `undo` runs undo; `finish_transaction` is never shown, because the UI itself refuses during a command the requests the core answers with it (3.7). A message with repairs also offers "Show what '{label}' lost", which opens repair mode (3.7). Copy details for a message with a `detail` (4.1). "Edit" opens the inspector, where the field of the first repair is the active selection field (3.7) |
| Sketch over-constraint while adding | Notification (3.16.6) or dimension editor (3.16.7) | "Replace them", "Add reference", "Cancel" |
| Sketch conflicts and extra constraints at rest | Sketch header, glyph and label colors, sketch problems popover; the sketch's `sketch_conflict` error with its actions (Problems panel, inspector) | "Delete" per constraint; "Select them" for free geometry (3.16.8); the error's actions ("Delete Distance 30 mm", "Make Distance 30 mm a reference") |
| Profile (outline) issues | Rings and halos in sketch mode, the header's outline suffix, the sketch problems popover with each finding's message and actions (3.16.8); warning notification on finishing | "Back to sketch"; in the popover the finding's actions ("Connect the ends (0.3 mm apart)") and "Show" |
| Extrude or Revolve with no closed shape | The disabled command's tooltip and, when clicked, its info notification (2.10) | `needs_closed_shape` offers "Show '{sketch}'" |
| Command failed in the core (`CoreError`) | Error notification with the core message | "Undo" when the error followed an edit that was applied; the error's `actions` other than `finish_transaction`; "Show problems" for `stale_fix`; Copy details when it has a `detail` |
| `focus_inactive` when editing | Error notification with the core message | "Unsuppress" or "Move marker to the end" |
| File, export and permission errors | Error notification (4.3) | "Save as", "Open..." |
| Core fatal | Fatal state (4.4) | "Restart core", "Reload" |
| Unsupported browser | Unsupported page | none |

Show (`runFixAction` of a `show` action): during a feature command, a `show` of the command's own feature frames its preview, and a `show` of another feature first ends the command as an implicit cancel (3.7). Otherwise it selects the feature (`feature:{id}`), or, when the action names an `entity` that the sketch still has, that entity's item (`sketch:{sketch}:{entity}` for a curve, the point item for a point entity), and frames what it made: the faces it created (3.6 feature highlighting, from their mesh positions), a sketch's entities in 3D, a datum plane's square, and the cause's ghost; a feature with none of these is selected only. When the action belongs to a cause line, `AppState.shown` keeps the cause's ghost, drawn on the overlay on the plane of the cause's sketch (its latest solved placement) as a dashed `errorMark` path (`data-testid="cause-ghost"`, `stroke-dasharray="4 3"`, SVG `title` `cause_ghost`, 4.3): the entity as it was before the edit, while the entity as it is now, when it still exists, is drawn selected. The ghost goes when the selection changes. In sketch mode on that sketch, the same is drawn in the sketch overlay.

### 4.3 UI-owned messages

`src/messages.ts` exports each of these as a function of its placeholders; tests compare exact text. A shortcut placeholder such as `{L}` or `{Mod+Enter}` is `formatShortcut` of that shortcut for the platform; `{key}` in `sketch_free_placement` is "Ctrl" on Windows and Linux and "⌘" on macOS; `{failed}` in `exported_with_failures` is the text of `export_failed_feature` or `export_failed_features`. Three take more than their placeholders' text: `drop_breaks_use(user, what, feature, dragged)` takes the labels and `what` (3.9), then `'user'` or `'feature'`, the one being dragged; `used_by(labels)` takes the users' labels, unquoted, in timeline order; `entity_used(user, what, entity)` takes the noun without its article (3.16.10), then the entity's type, `point`, `line`, `circle` or `arc`.

| Id | Text | Severity | Actions |
| --- | --- | --- | --- |
| `unsupported_webgl2` | Partsmith needs WebGL2 to draw the 3D view, and this browser does not provide it. Turn on hardware acceleration in your browser settings, or use a current version of Chrome, Edge, Firefox or Safari. | page | none |
| `unsupported_wasm` | Partsmith needs WebAssembly, and this browser does not provide it. Use a current version of Chrome, Edge, Firefox or Safari. | page | none |
| `core_load_failed` | The geometry core did not load: {reason}. Check your connection and reload the page. | fatal | Reload |
| `core_stopped` | The geometry core stopped: {reason}. Restart it to keep working; your latest changes are kept. | fatal | Restart core |
| `core_still_working` | The geometry core has been working for more than {seconds} seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it. | working panel | Stop and restart core |
| `core_still_working_desktop` | The geometry core has been working for more than {seconds} seconds, and Partsmith cannot tell whether it will finish. The desktop app cannot stop it yet: wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command. | working panel | none |
| `changes_not_applied` | Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them. | warning | none |
| `save_stopped` | {file} was not saved, because the geometry core stopped before saving it. Save it again. | warning | none |
| `export_stopped` | {file} was not exported, because the geometry core stopped before exporting it. Export it again. | warning | none |
| `open_stopped` | {file} was not opened, because the geometry core stopped before opening it. Open it again. | warning | none |
| `recovery_open_stopped` | The unsaved changes to {name} were not opened, because the geometry core stopped before opening them. They are still kept, and Partsmith offers them again the next time it starts. | warning | none |
| `new_stopped` | The new document was not opened, because the geometry core stopped before opening it. Choose New again. | warning | none |
| `not_ptsm` | {file} is not a Partsmith model. Partsmith opens .ptsm files. | error | none |
| `not_text` | {file} could not be read as text. | error | none |
| `open_failed` | Could not open {file}. {core message} | error | none |
| `migrated` | {file} was updated from format version {n}. Saving writes the current format. | info | none |
| `save_permission` | Could not save {file} because Partsmith no longer has permission to write it. | error | Save as |
| `save_not_found` | {file} was moved or deleted. Save it again to choose a location. | error | Save as |
| `save_io` | Could not save {file}: {reason} | error | Save as |
| `save_conflict` | {file} was changed elsewhere after you opened or saved it, so Partsmith did not save over it. | error | Overwrite {file}, Save as, Open the newer file |
| `save_assets` | {file} was not saved, because Partsmith could not write its imported files to the assets folder next to it. | error | Save as, Copy details |
| `save_folder_needed` | {file} is not saved yet, because its imported files are saved in the folder it goes in and the browser has not given Partsmith that folder. Choose the folder you are saving {file} in. | warning (persistent) | Choose folder |
| `download_hint` | This browser saves by downloading. {file} is in your downloads folder. | info | none |
| `recent_permission` | Partsmith needs permission to open {file}. Choose it again to grant access. | error | Open... |
| `recent_missing` | {file} has moved or been deleted, so it was removed from recent files. | warning | none |
| `assets_folder_needed` | The imported geometry of {file} is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder {file} is in. | warning (persistent) | Choose folder |
| `assets_wrong_folder` | {folder} is not the folder that holds {file}. Choose the folder {file} is in. | error | Choose folder |
| `assets_without_folder` | The imported geometry of {file} is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead. | warning | Open... |
| `assets_unsupported` | The imported geometry of {file} is missing, because this browser does not let Partsmith reach files next to the one you chose. Open it in Chrome or Edge, or in the Partsmith desktop app. | warning | none |
| `assets_not_read` | Partsmith could not read some imported files from the assets folder next to {file}, so their geometry is missing. | warning | Copy details |
| `assets_mismatch` | Some files in the assets folder next to {file} are not the files it imported, so Partsmith opened it without them and its imported geometry is missing. | warning | Copy details |
| `stashed` | {name} had unsaved changes. They are kept for you. | info | Reopen |
| `recovered` | Partsmith kept unsaved changes to {name} from {time}. | info (persistent) | Open, Discard |
| `recovered_more` | Partsmith kept unsaved changes to {name} from {time}, and to {n} other documents. ("and to 1 other document" for one) | info (persistent) | Open, Discard |
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
| `feature_lost` | '{label}' lost {description}. | warning, or error when a feature it names is in error | Show what '{label}' lost, the cause's first `apply` action, Undo |
| `features_lost` | '{label}' lost {description}, so {n} features after it failed too. ("so 1 feature after it failed too" for one) | error | Show what '{label}' lost, the cause's first `apply` action, Undo |
| `feature_check` | '{label}' needs a check after this change. | warning | Check '{label}', the cause's first `apply` action, Undo |
| `show_lost` | Show what '{label}' lost | action label | none |
| `check_feature` | Check '{label}' | action label | none |
| `repair_missing` | {Description} (missing) | SVG title | none |
| `repair_check` | {Description} (check it) | SVG title | none |
| `cause_ghost` | Before this change | SVG title | none |
| `needs_closed_shape` | '{sketch}' has no closed shape: {n} ends are not connected. ("1 end is not connected" for one) | disabled reason | Show '{sketch}' |
| `drop_breaks_use` | '{user}' uses {what} made by '{feature}'. Drop below '{feature}' instead? (dragging the user) or '{user}' uses {what} made by '{feature}'. Drop above '{user}' instead? (dragging the used feature) | drop hint | none |
| `used_by` | Used by {labels}, the labels in single quotes joined with ", ", and for more than three the first three followed by " and {n} more" | menu item, tooltip | selects the users (menu item) |
| `entity_used` | '{user}' uses the {what} made from this {line, circle or arc}. Draw its replacement in the same place and '{user}' will follow it. For a point: '{user}' uses the {what} made from this point. | hint (8 s) | none |
| `keep_sizes` | Keep sizes: add {unit} to the {n} numbers without a unit ("the 1 number without a unit" for one) | menu item | toggles Keep sizes |
| `field_limit` | Largest that works: {suggestion} | inline | none |
| `preview_pending` | Wait for the preview to update. | disabled reason | none |
| `details_copied` | Details copied. | live region | none |
| `details_not_copied` | The details could not be copied, because the browser did not allow access to the clipboard. | warning | none |
| `reattach_prompt` | Pick a plane or a flat face for '{label}' | hint | none |
| `attach_label` | Attach '{label}' | undo label | none |
| `attach_after` | '{label}' cannot be placed on '{feature}', which comes after it. Pick an origin plane, or a flat face or datum plane made before '{label}'. | hint | none |

### 4.4 Fatal states

- Core load failure (`transport.start()` rejects, including a protocol version mismatch reported by the worker host): the viewport shows a panel (`data-testid="core-fatal"`, `role="alert"`) with `core_load_failed` and a "Reload" button (`location.reload()`); the rest of the shell stays but every core-dependent command is disabled with "The geometry core is not running."
- Core stopped later (fatal listener): the same panel with `core_stopped` and "Restart core". `app.restartCore()` creates a new transport and client, starts it, reloads the catalog and printers, and reopens the last recipe with `openDocument(JSON.stringify(recipe, null, 2), assets)`, `assets` being the session's `DocumentState.assets` (the asset files it gave the stopped core, 3.19, so an import keeps its geometry without reading the folder again), keeping the document name, file reference (handle, folder and digest), assets and `recoveryId` (so its recovery entry carries on and is written at once, 3.19) and setting `localUnsaved` true. Before it stops the old client, it discards the mutations still waiting in the queue (`mutations.discard(new CoreFatalError('The geometry core was stopped.'))`), so none of them reaches the new core. The recipe is the settled recipe (1.3), so the edits of an open command (its preview, which may be what stopped the core) are never reopened as permanent features. A restart that fails keeps that recipe, name, file reference and assets for the next attempt; with no document open, the restart opens a new "Untitled" document. Undo history is lost; selection, command and sketch session are cleared.
- Core still working: only requests the user waits on count. A request sent through `app.core` is a background request when its command is one the UI sends on its own to keep what it shows current: `analyze_print` (3.17), `list_elements` (hover and highlight lists, 3.6), `resolve_references` (selection upkeep and highlights), `measure` (3.8), `evaluate` (2.13) and `list_uses` (3.9, 3.16.10), the set `BACKGROUND_COMMANDS` of `src/app/createApp.ts`. Every other request is waited on, among them every task of the mutation queue (changes, the previews of an open command, open, new, close, save, mark saved and export, 2.5), `pick_reference`, `solve_sketch` and `drag_sketch`. When the core has answered nothing for `STILL_WORKING_SECONDS` (10, `src/app/createApp.ts`) while at least one waited-on request waits for its response, counted from the later of the oldest such request's sending and the core's latest response to any request, background ones included, `core.stillWorking` becomes true; the core's next response, a fatal error or a restart sets it false. A background request starts no count, so a slow one alone never shows the panel or offers Stop: a long print check shows only on its own controls, as the health badge's `running` state ("Checking" with its spinner) and, before the first report, the print check panel's "Checking the model for {profile name}." (3.17), and `status-core` does not change. The core answers in request order, so a waited-on request sent behind a slow background one counts from its own sending and shows the panel once it has waited 10 s, whatever the core is busy with. A user action that waits on background requests before it goes on counts that wait as a waited-on request sent when the action starts (`app.waitOn`, 2.9): Enter in a sketch draw input waits on the evaluations of the typed values (3.16.4), and a face picked as its edges on the edge list (3.7), so a core that never answers them shows the panel 10 s after the Enter or the pick.
- While `core.stillWorking` is true, the viewport shows, in the place and style of the fatal panel but over the model, the panel `data-testid="core-working"` (`role="alert"`, as the fatal panel, so assistive technology announces it as it appears) with `core_still_working` (`{seconds}` is 10) and the primary button "Stop and restart core", and `status-core` reads "Still working" (3.21). The panel is drawn beside the viewport's content, never in its place: it is a card centered in a layer over the free area (3.2) (`pointer-events-none absolute inset-0 z-30`, with only the card `pointer-events-auto`; `z-30` is above the inspector's `z-20`, so an inspector placed or dragged over the card never hides its text or button, and the layer is inside the free-area layer, which is `isolate`, so the browser and side panel overlays and the notifications, drawn over the viewport, stay above the panel), so the viewport (`data-testid="viewport"`) keeps its canvas, view cube, viewport controls, top-center slot, legends and inspector mounted, the model stays drawn and can be orbited, panned and zoomed around the chip, and every other region, the top bar's context switch and toolbar included, keeps working. Nothing stops on its own, and edits made meanwhile queue as usual.
- "Stop and restart core" runs `app.restartCore()` while the core is still working. It first notes whether a change was unfinished: a `change` task was started or waiting in the mutation queue (`mutations.changing`, 2.5), or a feature command was open. An `other` task, such as the `export` that `exportModel` sends through the queue to see every edit before it, does not count. Then it restarts as above: the waiting mutations are discarded, stopping the transport terminates the worker and rejects the request in flight with `CoreFatalError`, and the settled recipe is reopened without the open command (no `cancel_transaction` is sent and no `command_canceled` is shown). Once the document is open again, it shows `changes_not_applied` as a warning when it noted an unfinished change. A change that `CoreFatalError` rejects shows nothing itself, while an action whose `other` task it rejects says at once, as a warning, what was not done: Save, Save as and the "Choose folder" of `save_folder_needed` `save_stopped` and Export `export_stopped` (`{file}` the target's name), Open, Open recent, a dropped file, "Open the newer file" and the "Choose folder" that reopens a document with its assets (3.19) `open_stopped`, a recovery Open or Reopen `recovery_open_stopped`, and New `new_stopped`. A `mark_saved` rejected after the file was written shows nothing and leaves the document unsaved. These warnings also follow a core that stops by itself (the fatal state above). The desktop core runs inside the app's process, where nothing can stop a running call (desktop.md open issue 9), so with the Tauri transport the panel shows `core_still_working_desktop` and no button.
- A `CoreError` with code `internal_error` or `document_poisoned` is shown verbatim as an error notification with no other change; the core's message already tells the user what is safe and what to do, and an `internal_error`'s notification offers Copy details (4.1), since its message asks for a report with the details.
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

UI budgets derived from them (same runner, except initial JavaScript, which depends only on the build):

| Interaction | Budget | Measured as |
| --- | --- | --- |
| Pick query with a current ID pass | 1 ms | 95th percentile of `ps:pick` |
| Hover highlight | next animation frame | `ps:pick` plus one frame |
| Palette re-rank per keystroke, 2,000 items | 8 ms | 95th percentile of `ps:palette` |
| Expression validation round trip (web) | 50 ms | Median of `ps:expression`, from the keystroke to the field showing the result |
| Main-thread long tasks during orbit, drag or preview typing | none over 50 ms | `PerformanceObserver` of `longtask` |
| Applying a `document_changed` plus `recomputed` for 50 features | 8 ms | 95th percentile of `ps:apply-events`, around the store updates |
| Timeline drop check per pointer move while dragging a feature (3.9), over the `list_uses` result of a 300-feature document | 1 ms | 95th percentile of `ps:drop-check` |
| Initial JavaScript: the scripts `index.html` loads and the core worker script, excluding the core module | 250 KB gzipped (250,000 bytes, gzip level 9) | `pnpm check:dist` on every CI build (delivery.md 2.6) |

Rules that make these reachable: hot paths bypass React; meshes are never copied on the main thread (frame views go straight to `upload`); picks read a small block of the ID pass; previews and drags are latest-only; analysis never runs automatically outside Print.

## 6. Acceptance tests

Vitest unit tests live next to the code as `src/**/*.test.ts(x)` (jsdom; no WebGL; fakes from `src/test/fakes`). Playwright end-to-end tests live in `ui/e2e/*.spec.ts` and run against the production build with the real WebAssembly core in `ui/public/core/` (`pnpm -C ui e2e`; every E-test runs in the `chromium` project, Desktop Chrome, and the tests tagged `@engines` also in the `firefox` and `webkit` projects, which run only when `--project` names them, 6.8; every project has a 1280 by 720 viewport at device scale factor 1 on Linux, so `Mod` is Control; use Playwright's `ControlOrMeta`). Protocol-level codec, bridge and worker host tests are protocol.md tests 53 to 66 and are not repeated here. Next to the tests, every change passes `pnpm -C ui lint` (`oxlint --deny-warnings`, then `oxfmt --check`) and `pnpm -C ui typecheck` (`tsc -b`); test files are linted like other code (oxlint's `vitest` plugin), and `src/generated` is excluded from both tools.

### 6.1 Selectors

Tests use these stable selectors only (plus accessible roles and names given in section 3).

| `data-testid` | Element | Attributes |
| --- | --- | --- |
| `app` | Root | `data-core-status` (`loading`, `ready`, `failed`), `data-context` (`model`, `sketch`, `print`) |
| (none; `html` element) | Document root | class `dark` in the dark theme (2.16.8) |
| `unsupported` | Unsupported page | |
| `core-overlay`, `core-working`, `core-fatal` | Loading overlay, still-working panel, fatal panel | |
| `top-bar`, `document-name`, `save-state`, `document-menu`, `palette-button`, `theme-toggle` | Top bar parts | `document-name`: `data-dirty`; `theme-toggle`: `aria-pressed` |
| `palette`, `palette-input`, `palette-option`, `palette-shortcut`, `palette-heading`, `palette-empty`, `palette-footer` | Palette | option: `data-item-id`, `data-kind` |
| `browser`, `browser-row` | Browser | row: `data-kind`, `data-id` |
| `color-popover` | Body color popover | |
| `viewport` | Viewport; its box is the free area (3.2), while its canvas fills the window | `data-hover-kind`, `data-hover-key`, `data-selection-count`, `data-view`, `data-projection`, `data-nav-preset`, `data-section`, `data-camera-target`, `data-grid-spacing`, `data-split-planes` |
| `view-cube`, `viewport-controls`, `section-popover`, `viewport-menu`, `empty-state`, `plane-label`, `repair-marker`, `repair-ghost`, `cause-ghost` | Viewport chrome | |
| `top-center-slot` | The top-center slot's column (3.2) | |
| `axis-label` | Axis label at an origin axis's positive end (3.12), or where a sketch axis on an origin plane leaves the free area (3.16.2) | `data-axis` (`X`, `Y`, `Z`) |
| `axis-letter` | Axis letter in a coordinate field or readout (3.7, 3.8) | `data-axis` (`X`, `Y`, `Z`); in fields `data-world` (`true`, `false`) |
| `repair-candidate` | Numbered candidate marker in the viewport (3.7) | `data-number` |
| `repair-candidate-button` | Numbered candidate button in the inspector banner or the re-attach header (3.7, 3.16.1) | |
| `cause-line` | A cause line under a message (4.1) | |
| `field-limit` | "Largest that works" line under a field (3.7) | |
| `drop-hint` | The timeline's drop warning (3.9) | |
| `problem-row`, `problems-notes` | Problems panel rows and the Notes disclosure (3.9) | row: `data-feature-id`, `data-nested` |
| `entity-used` | Used mark of a sketch entity (3.16.10) | `data-entity` |
| `keep-sizes` | Keep sizes item of the units menu (3.21) | `aria-checked` |
| `inspector` | Inspector | `data-mode` (`command`, `selection`), `data-preview` (`none`, `pending`, `shown`, `failed`) |
| `inspector-field`, `inspector-more`, `inspector-ok`, `inspector-error` | Inspector parts | field: `data-path`; error banner: `data-severity` |
| `selection-field` | Selection field box | `data-active`, `data-count` |
| `expression-result`, `field-error`, `expression-suggestions` | Expression field parts | |
| `selection-title`, `body-size`, `body-center`, `vertex-position`, `entity-size`, `measure-row`, `made-by` | Selection inspector parts | measure row: `data-measure`; `body-size`, `body-center`, `vertex-position`: `data-value` (the plain `formatSize` or `formatPoint` text, 3.8) |
| `timeline`, `timeline-item`, `timeline-marker`, `timeline-problems`, `context-menu`, `timeline-empty`, `insertion-bar` | Timeline | item: `data-feature-id`, `data-status`, `data-suppressed`, `data-rolled-back`, `data-notes` |
| `side-panel`, `parameters-panel`, `parameter-row`, `parameter-value`, `parameters-empty`, `parameters-transaction`, `problems-panel`, `health-list`, `health-item`, `health-suggestion`, `keys-panel` | Side panels | row: `data-id`; health item: `data-check`, `data-severity`; `health-suggestion` is the suggestion line inside a health item (3.17) |
| `sketch-header`, `sketch-problems`, `sketch-hint` | Sketch chrome | header: `data-state`, `data-dof`, `data-outline` |
| `sketch-dimension`, `dimension-editor`, `constraint-glyph`, `inference-glyph`, `draw-input`, `profile-issue`, `box-selection` | Sketch overlay | dimension: `data-constraint-id`, `data-type`, `data-driving`; glyph: `data-constraint-id`, `data-type`, `data-state`; inference: `data-type` |
| `status-bar`, `status-core`, `status-dof`, `status-grid`, `status-recompute`, `status-units`, `status-printer`, `health-badge` | Status bar | dof: `data-dof`, `data-state`; badge: `data-state`, `data-stale` |
| `printer-menu`, `checks-popover`, `overhang-legend`, `finding-marker`, `print-hint`, `export-popover`, `export-status`, `drop-overlay` | Print and export | |
| `notification` | Notification | `data-severity`, `data-code` |
| (any) | Element positioned at runtime (3.12) | `data-positioned`; its `style` attribute holds only `translate` (the view cube rotor: only `transform`) |

### 6.2 Fixtures

`ui/e2e/fixtures/` holds `.ptsm` files written by the test author from model.md, sketch.md and the feature specs. Every fixture is canonical (`partsmith fmt` leaves it unchanged), valid (`partsmith validate` is clean, except for the sketch problem that `nc07.ptsm` and `nc08-broken.ptsm` state, the misspelled parameter of `nc10.ptsm`, and `step-block.ptsm`, which validates with `asset_missing` because its asset stays in `examples/import/assets/`), unit `mm`, with unnamed features (labels come from features.md 3.12). The fixtures from `bevel-corner.ptsm` on hold scenarios of [the reference stability corpus](../design/reference-stability-corpus.md) and [the newcomer error corpus](../design/newcomer-error-corpus.md): they are written at format version 2 with every reference and region picked as the corpora's "Picked references" rule says (anchors, merged names, region outlines, `fallback_frame: body`), so saving one unchanged gives its bytes back. E-109 to E-126 open them and check their stated results; E-108 covers the fixtures above them, and E-150 and E-154 check the result of `step-block.ptsm`.

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
| `hollow-resin.ptsm` | Body `b-hollow` "Hollow": `f-box1` box `20 mm` × `20 mm` × `20 mm` (x and y in [-10, 10], z in [0, 20]), op new, then `f-sh1` shell `2 mm` with no faces removed (a closed hollow, x and y in [-8, 8], z in [2, 18]); `printer` `generic-resin`. | A `trapped_volumes` warning (`enclosed_hollow`) with a drain hole suggestion at z = 2 |
| `notes.txt` | The text `hello`. | Not a model |
| `sketch-500.ptsm` | One sketch with at least 500 constraints in one connected component and DOF at least 2, where dragging point `e-drag.end` moves at least 100 other entities. | Perf |
| `part-50.ptsm` | A copy of `examples/bench/fifty-features/model.ptsm` (engine.md section 5): 50 features whose first sketch is driven by parameter `base`. | Perf |
| `dense.ptsm` | At least 2,000,000 display triangles at standard quality (for example a 20 by 20 linear pattern of spheres). | Perf |
| `palette-2000.ptsm` | 1,980 parameters `p0` to `p1979` and 20 features. | Perf |
| `part-300.ptsm` | 300 features in one body: 60 groups of a sketch, an extrude of it (op join, the first op new), a fillet on two of that extrude's edges, a face sketch on its top, and a datum plane on that top. | Perf |
| `bevel-corner.ptsm` | Corpus R03's setup with `e-2` ending and `e-3` starting at (80, 50), so that the bevel line `e-5` from (80, 40) to (70, 50) cuts the corner: `f-ext1` extrudes the five-sided region bounded by `e-1`, `e-2`, `e-5`, `e-3` and `e-4`, distance 30, op new, body `b-1` "Body 1"; `f-fil1` fillet radius 2 on the top edge of the bevel face and the front top edge. Deleting `e-5` leaves the full rectangle, the corpus edit made in one step. | Timeline "Sketch 1", "Extrude 1", "Fillet 1", each ok |
| `rabbet.ptsm` | Corpus R16 with its edit suppressed: CUBE20 (`f-box1` box 20 × 20 × 20 centered on the origin in x and y, op new, body `b-1` "Body 1"), then `f-sk2` on YZ with the rectangle y -11 to -5, z 15 to 21, then `f-ext2` extrude of it, symmetric 30, op cut, `suppressed: true`, then `f-fil1` fillet radius 2 on the front top edge of `f-box1`. | Timeline "Box 1", "Sketch 1", "Extrude 1" (suppressed), "Fillet 1" ok |
| `pattern-fillet.ptsm` | Corpus R41: CUBE10 (`f-box1` box 10 × 10 × 10, body `b-1` "Body 1"), `f-lp1` linear pattern of `b-1` along origin X, count 3, spacing 20, op join, and `f-fil1` fillet radius 1 on the top right edges of the second and third cube. | Each feature ok |
| `boss-fillet.ptsm` | Corpus R28: BLOCK, `f-dp1` datum XY offset 30, `f-sk2` on it with circle `e-5` at (40, 25) radius 10, `f-ext2` extrude of it, distance 10, op join, and `f-fil1` fillet radius 2 on the boss's top rim. | Timeline "Sketch 1", "Extrude 1", "Datum plane 1", "Sketch 2", "Extrude 2", "Fillet 1", each ok |
| `side-line.ptsm` | Corpus R06's BLOCK with `f-fil1` fillet radius 2 on the right top edge (the edge along `e-2` at x = 80), without R06's face sketch and pocket. | Each feature ok |
| `nc01.ptsm` to `nc06.ptsm`, `nc09.ptsm` to `nc12.ptsm` | The setups of newcomer error corpus scenarios NC01 to NC06 and NC09 to NC12, one file each (NC04 is NC03 with op join). | The corpus's "Expect" for NC01 to NC06, NC10 and NC12; NC09 and NC11 each feature ok before their edit |
| `nc07.ptsm` | NC07's setup with its edit already made: `c-1` Distance "40 mm" and `c-2` Distance "30 mm" on `e-1`, because the dimension editor refuses a dimension that conflicts (3.16.7). | NC07's "Expect" |
| `nc08.ptsm`, `nc08-broken.ptsm` | NC08's setup (the 30, 40, 50 triangle), and the same with `c-3` set to "80 mm". | `nc08.ptsm` each feature ok; `nc08-broken.ptsm` NC08's error, without a cause line |
| `step-block.ptsm` | A copy of `examples/import/step-block.ptsm`, kept byte for byte identical to it as `part-50.ptsm` is to its example: body `b-1` "Body 1" and `f-imp1`, an import (op new) of `S`, the import track's asset `examples/import/assets/b8d947e1aae9d107df35bea403414dc9ae3df630b84426c70e92770948bdfdb4.step` (15,303 bytes, the 10 by 20 by 30 "Block" of features/import.md 6). The E-tests read `S` from there; it is not copied. | With `S` in the `assets/` folder next to it: "Import 1" ok and "Body 1" 10 × 20 × 30 mm; without it, "Import 1" fails with `asset_missing` |

### 6.3 Helpers (`ui/e2e/helpers.ts`)

- `openApp(page, { fileSystemAccess = true } = {})`: when false, an init script deletes `window.showOpenFilePicker`, `window.showSaveFilePicker` and `window.showDirectoryPicker` (Firefox and WebKit have none of them, so there the option changes nothing, and E-4, E-6 and E-8 take the paths of a browser without them as they are written); then it turns on reduced motion (`page.emulateMedia({ reducedMotion: 'reduce' })`), so camera moves are instant (2.11) and a click right after a view change lands on the final view; then `page.goto('/')` and `expectReady(page)`.
- `expectReady(page)`: waits until `app`'s `data-core-status` is `ready`, for at most `OPEN_TIMEOUT_MILLISECONDS` (60 s, less when the test's own timeout comes first); `openApp` and every test after a reload or a core restart use it. It polls the state the page shows: the unsupported page (`unsupported`) with its text, `app` not rendered yet, the status `loading` or `ready`, or `failed` with the fatal panel's text (`core-fatal`, 4.4). The unsupported page fails it at once with that page's text, since nothing starts after it (3.1). Every other state is read again until `ready`, so a core that becomes `ready` after any state in between passes, and a wait that ends without `ready`, at its own timeout or at the test's, fails with the last state it read (`expect.poll` reports it at either timeout), which names what the app showed: on the first platforms run WebKit's tests waited 30 s on `failed` with nothing to say why (open issue 41).
- `dropFile(page, name)`: reads the fixture, builds a `DataTransfer` with a `File` in the page, and dispatches `dragenter`, `dragover` and `drop` on the app root; waits for `document-name` to equal the name without extension and for a new `ps:open` measure, the first frame of the opened document (2.18), or for a notification when the file is not a `.ptsm`.
- `canvas(page)`: the view the tests aim at, which is the free area (3.2): the bounding box `{ x, y, width, height }` of `viewport`, its center `(cx, cy)` and `H = height`. The canvas element itself fills the window under the islands, but fit, the standard views and zoom center the model in the free area and scale it to the free area's height (2.11), so the formulas below hold in this box.
- `fitView(page, view)`: clicks "{View} view" on the cube, turns "Orthographic" on, clicks "Fit to view", and waits for `data-view` and `data-projection`.
- `toScreen(box, bounds, view)`: maps model millimeters to page pixels using the fit rule of 2.11 (content extents scaled to 80 percent of the free area's limiting dimension, bounds center at the free area's center). For the cube in the front view: `s = 0.04 × H` pixels per mm, `pageX = cx + x × s`, `pageY = cy - (z - 10) × s`; the top edge of the front face is at `top = cy - 0.4 H`.
- `sketchToScreen(box, contentMin, contentMax)`: the same mapping for a sketch on XY just entered (top view, content framed with margin 0.1).
- `expectHover(page, x, y, kind)`: moves the mouse and polls `data-hover-kind`.
- `download(page, action)`: waits for the `download` event during `action` and returns the suggested name and bytes.
- `useFolders(page)`: called before `openApp`. An init script replaces `showOpenFilePicker`, `showSaveFilePicker` and `showDirectoryPicker`, on every load of the page, with pickers over the page's origin private file system (`navigator.storage.getDirectory()`, whose handles Chromium answers every permission query for with `granted`, and whose `createWritable`, `resolve` and `removeEntry` behave as for picked files). They answer from a queue that `queuePick(page, path)` fills with slash-separated paths from that root, one per picker call: `showOpenFilePicker` resolves `[the file handle at path]`, `showSaveFilePicker` the handle of the file at path, created empty when missing (as Chromium's save dialog creates the file it names), and `showDirectoryPicker` the folder handle at path. Each call is recorded with its options, a handle given as its `name` (`pickerCalls(page)` returns the calls of the current load in order), and a call with the queue empty rejects with a `DOMException` named `AbortError`, as a canceled dialog does.
- `writeFolderFile(page, path, bytes)` writes the bytes to the file at path in the origin private file system, creating its folders; `readFolderFile(page, path)` returns that file's bytes, or null when there is none; `folderNames(page, path)` returns the sorted names of a folder's entries. The context of each test starts with an empty origin private file system, which a reload keeps.
- `stallCore(page, marker)`: called before `openApp`. It routes the core module URL `/core/partsmith_core.mjs` without a query (`page.route`, which in Chromium also serves the core worker's requests) to a module whose default export calls the real factory (imported from `partsmith_core.mjs?real`) and resolves with the instance it made after replacing that instance's `_ps_call`: a request frame whose bytes contain `marker` in UTF-8 never returns (an endless loop, as a core stuck in the kernel would), and every other request calls the real `_ps_call`.
- `measures(page, name, since = 0)`: the durations of the entries of `performance.getEntriesByName(name)` whose `startTime` is at least `since`, oldest first.
- `pageTime(page)`: the page's `performance.now()`, the `since` of `measures` and `longTasks` for an interaction that starts next.
- `percentile(values, fraction)`: the nearest-rank percentile: the values sorted ascending, the one at index `ceil(fraction × n) - 1`; it throws for an empty list. The median is `percentile(values, 0.5)` (the lower middle value of an even count) and the 95th percentile `percentile(values, 0.95)`.
- `watchLongTasks(page)`: adds an init script that, on every load of the page and before the page's own scripts run, observes `longtask` entries with a `PerformanceObserver` and keeps each entry's `startTime` and `duration` on `window`. Called before `openApp`.
- `longTasks(page, since = 0)`: the durations of the long tasks `watchLongTasks` saw that started at or after `since`, oldest first.

The performance tests run only with `PARTSMITH_PERF=1` (6.7), so `smoke.spec.ts` checks the helpers only they use on every run, without `@perf`: `percentile` gives 5 for [5] at 0.95, 10 for 1 to 10 at 0.95 (in any order) and for 1 to 20 at 0.5, 4 for [7, 1, 6, 2, 5, 3, 4] at 0.5 (the 4th of 7, which `median` of `partsmith-budgets` takes, engine.md section 5), and throws for an empty list; after `watchLongTasks` and `openApp`, a 120 ms busy loop in a task of the page (a `setTimeout` callback; script that `page.evaluate` runs directly is not a page task) run after a `pageTime` gives a long task of at least 100 ms in `longTasks` from that time and none from a `pageTime` read after the loop; and `measures` from that first `pageTime` leaves out a measure recorded before it.

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
62. U-62. Sketch view history: enter (view entry), orbit (view entry), commit an edit (document marker); three `Mod+Z` presses pop the newest entry first (3.20): they call core `undo` once, restore the camera from before the orbit, and restore the pre-entry camera, in that order; `Mod+Shift+Z` replays them in reverse.
63. U-63. Sketch header mapping: no entities `empty`; `under_constrained` with dof 3 `under` and "Not fully constrained · 3 DOF"; `fully_constrained` `full`; status `conflicting` with 2 conflicting ids `conflict` and "2 conflicting constraints"; `failed` true with no conflicting ids `failed` and "Cannot solve"; status `redundant` with dof 0 and one redundant id `full` with " · 1 extra constraint" appended; two `open_endpoint` findings with dof 0: `full`, `data-outline` 2 and " · 2 open ends" appended; an `open_endpoint` and an `overlap`: " · 2 outline problems".

Store and actions:

64. U-64. `startFeatureCommand('extrude')` with a preselected region and no bodies: `beginTransaction(documentHandle, 'Extrude')`, then one `applyEdits(documentHandle, null, [add_body "Body 1", add_feature])` with the body id in the record, then `setFocus(documentHandle, featureId)`.
65. U-65. With `rollback` set to the second of 3 features (index 1), the first preview's `add_feature` has index 1 and no `set_rollback` follows.
66. U-66. `confirmCommand` calls `commitTransaction`; `cancelCommand` calls `cancelTransaction`; with an empty command history, `Mod+Z` cancels the command and calls no `undo`; after two picks, `Mod+Z` removes the second pick and calls neither `cancelTransaction` nor `undo`, and `Mod+Shift+Z` restores it.
67. U-67. `editFeature('f-ext1')` calls `beginTransaction(documentHandle, 'Edit Extrude 1')` and `setFocus(documentHandle, 'f-ext1')`; a `focus_inactive` error on a suppressed feature calls `cancelTransaction` and shows an error notification with the action "Unsuppress".
68. U-68. Timeline edits: deleting the rollback feature sends `set_rollback` to the nearest later remaining feature (no id when none) before `remove_feature`; dropping feature index 3 at gap 1 sends `move_feature` index 1, at gap 4 index 3 (no edit); `setRollback(N)` sends `set_rollback` without an id, `setRollback(2)` the id of the third feature and `setRollback(0)` the id of the first.
69. U-69. Esc ladder: with an open popover, a sketch drawing in progress, an active tool, a command and a selection, successive Esc presses close the popover, cancel the drawing, exit the tool, cancel the command, then clear the selection.
70. U-70. Contrast: for every row of the text and non-text tables in 2.16.6 and each theme, `contrastRatio` of the two roles' hex values (`PALETTE_HEX` of the steps in 2.16.3 and 2.16.4; `warningMark` is `zinc-950` light and `yellow-400` dark, and its border and background rows `yellow-800` light and `yellow-300` dark; a role with an opacity, such as the dark `hover` (`zinc-700` at 60 percent), composited over the role named after "over" per sRGB-encoded channel, rounded to 8 bits) truncated to two decimals equals the table's value and is at least 4.5 for text rows and 3 for non-text rows; for every row of its measured table, the ratio computed the same way after compositing as that table says (a percentage mixes the color over `viewport`, and "over" over the role it names, per sRGB-encoded channel, rounded to 8 bits; a hovered or selected body mixes `highlight` into `body` at 20 or 35 percent) equals the table's value; `contrastRatio('#ffffff', '#000000')` is 21.
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
86. U-86. Save to a handle calls `saveDocument`, `files.readHandle` (the change check of 3.19, for a file opened from that handle), `files.write`, then `markSaved` with the returned revision, in that order.
87. U-87. During an edit, a status message `{severity: 'error', code: 'fillet_too_large', field: '/radius', suggestion: '2.3 mm', actions: [{label: 'Use 2.3 mm', run: {kind: 'apply', fix: <FixRef>}}]}` shows under the `radius` field and its action "Use 2.3 mm" calls `applyFix` with that `FixRef`; when the fake core's events give the record `radius` `2.3 mm`, the command's `radius` is `2.3 mm` and the radius input has focus; a message with `field: '/edges/1'` maps to the `edges` field.
88. U-88. Double-click on a face whose descriptor has `feature: 'f-ext1'` calls `editFeature('f-ext1')`.
89. U-89. With an active feature command, the export commands are disabled with `finish_command_first`; the timeline item of an `import` feature offers Rename, Suppress and Delete, and its Edit is disabled with `import_not_editable`.

Theme, styling and cross-spec helpers:

90. U-90. Mapping table and viewport values agree: (1) every `PALETTE_HEX` value equals the conversion of 2.16.5 (CSS Color 4 OKLCH to linear sRGB, per-channel clip, sRGB encoding, 8-bit rounding) of that step's OKLCH string from `tailwindcss/colors` (`white` from `#fff`, `black` from `#000`); (2) `COLOR_CLASSES` equals the table of 2.16.3, held in the test as data (role, facet, class string), and every color class in it names a step of `PALETTE_HEX` (an opacity modifier such as `/40` aside), except `forcedEdge`'s `transparent`; (3) `VIEWPORT_STEPS` equals the table of 2.16.4 (including `grid`), and `VIEWPORT_COLORS[theme][role]` equals both `PALETTE_HEX[VIEWPORT_STEPS[role][theme]]` and the table's hex; (4) `viewport` has the steps of `stage`'s background, `highlight` those of `accent`'s background, `problem` and `sketchOver` those of `errorMark`'s foreground, `body` the Default swatch's step, and the `analysis` classes name the steps of `analysisLow` and `analysisHigh`; (5) `BODY_SWATCHES` equals the swatch table of 2.16.5, each `background` is `bg-{step}` and each non-null `hex` is `PALETTE_HEX[step]`; (6) `viewportTheme('dark').highlight` equals `linearFromHex('#ff6900')` and `viewportTheme('light').background` `linearFromHex('#f4f4f5')`, within 1e-12; `UI_SCALE_CLASSES` and `ROOT_CLASSES` equal 2.16.9 and 2.16.2, and `roles.ts` exports no `FONT_CLASSES` (the faces are theme variables, 2.16.7).
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
100. U-100. `capturePick` in a Hole command with a flat-face item (placement origin (0, 0, 20), x axis (1, 0, 0), normal (0, 0, 1)) and a `PointerRay` hitting (3, 4, 20), far from the centroid: the command gets a helper sketch with plane kind `face`, the descriptor's body and reference, the descriptor's `fallback` as `fallback` and `fallback_frame` `body`, the descriptor's `placement` as its `placement`, and one point entity at [3, 4]; `points` is its `PointRef`, `sketch` the helper id and `body` the face's body. A second click at (-2, 1, 20) adds a second entity. A click on another face leaves the params unchanged and shows `hole_one_face`. With `points` holding points of `f-sk2`, a face click replaces them with the helper's point. A ray within 8 times `millimetersPerPixel` of the centroid places [0, 0].
101. U-101. Hole preview edits with a fake core: the first application sends, in one `applyEdits`, `add_feature` of the helper at index n and `add_feature` of the hole at index n + 1; the next sends `update_feature` of both; after Points is replaced by points of `f-sk2`, the next sends `remove_feature` of the helper; `cancelCommand` calls `cancelTransaction` and nothing else.
102. U-102. `sketchStepHint` on Linux: `{kind: 'idle', empty: true}` gives "Choose Line (L), Rectangle (R) or Circle (C) to start drawing."; `{kind: 'idle', empty: false}` gives null; line before a click "Click to start a line."; line with a chain "Click to end the segment, or type a length. Click the first point to close the shape, or press Enter to stop." (on macOS it ends "press ↩ to stop."); circle after click 1 "Click to set the size, or type the diameter."; dimension with a point picked "Click a second point or a line."; with a circle picked "Click to place the dimension.". `sketch_free_placement` renders "Hold Ctrl to place freely." on Linux and "Hold ⌘ to place freely." on macOS.
103. U-103. `defaultParams` with the test catalog in a document without features: fillet `radius` `1 mm`; chamfer `distance` `{kind: 'equal', expr: '1 mm'}`; shell `thickness` `2 mm`; hole `diameter` `3.2 mm` and `extent` `{kind: 'through_all'}`; revolve `extent` `{kind: 'full'}`; datum plane `offset` `10 mm`; extrude as in U-33. In a document whose latest fillet has `radius` `edge_r`, a new fillet starts at `edge_r`, and its `edges` and `body` do not carry over; when that latest fillet's status is `error`, the radius of the latest fillet that is not in error is used. A fillet's `body` starts empty and a hole's starts at `lastTargetBody`. `setParam` of `hole_type.kind` to `counterbore` with diameter `3.2 mm` gives `hole_type` `{kind: 'counterbore', diameter: '6.4 mm', depth: '3 mm'}`; with diameter `screw` the head diameter is `2 * (screw)`; `countersink` gives angle `90 deg`; `setParam` never reads the document.
104. U-104. With a Fillet command active (edges set) and the viewport focused, the keys `2` and Enter make `confirmCommand` commit params with `radius` `2`. With Extrude active, `.5` gives `extent.expr` `.5`; after the user edited Opposite distance, `3` goes there. `e` types nothing and stays a shortcut. Tab on the viewport focuses the first essential expression field with its text selected. Fillet started with one preselected edge has `activeField` `edges` and focus in Radius. In Revolve, a profile pick advances the active field to `axis`. Box started in a document without features (op `new`, empty `body`) has no active field and focus in Length with its text selected.
105. U-105. Split body with `body` `b-cube` and `plane` origin XZ preselected: the first `applyEdits` is `[add_body "Body 1", add_feature]` with `target` the new body's id; clearing Target body makes the next application send `update_feature` without `target`, and `remove_body`. Mirror with op `join` has no `target` field in the model; setting op `new` shows it as "Body 1 (new)", and the first application adds that body; setting op back to `join` removes it. Extrude with op `new` still creates its body in `body`.
106. U-106. Extrude whose profile sketch is on a face of `b-cube`, with `body` `b-cube`: `setCommandField('op', 'cut')` sets `reverse` true and `'join'` sets it false. After `setCommandField('reverse', false)` while op is cut, later op changes keep it. With the sketch on origin XY, op changes never touch `reverse`.
107. U-107. An Extrude command with `extent.expr` `-5 mm` and the status message `{severity: 'error', code: 'out_of_range', field: '/extent/expr'}` shows the action "Reverse and use 5 mm", which sets `reverse` true and `extent.expr` `5 mm` and focuses the Distance input. With `extent.expr` `-wall` there is no such action, nor for a Hole whose `diameter` `-3 mm` has the message on `/diameter`. A `no_effect` warning with op `cut` whose actions hold `Turn on Reverse` shows that action, which calls `applyFix`, and once the record has `reverse` true focuses the Distance input; without that action no Reverse action is shown (U-141).
108. U-108. With an Extrude command active, `updateParameter(app, 'p-size', {expression: '30 mm'})` calls `applyEdits(documentHandle, null, [update_parameter])` and sets `preview` to `pending` until its events arrive; without a command, the label is "Change size". During the command, `file.save`, `edit.suppress` and `document.units.in` are disabled with "Finish or cancel Extrude first.", and `view.section` is enabled.
109. U-109. A Fillet command with two picks, then `startFeatureCommand('chamfer')`: `cancelTransaction` is called and the notification "Fillet was canceled." offers "Restore". Running Restore cancels the Chamfer command without another notification and reopens Fillet with both edges and the same active field. A command with an empty history that ends the same way shows no notification, nor does a command with picks ended by `newDocument`. With a Box command's preview in a clean document, `newDocument` and `openFile` send `cancel_transaction` before `close_document` and write no recovery entry.
110. U-110. With a fake core: in a sketch session, an applied sketch edit after which `f-fil1` goes from ok to error raises no notification; `finishSketch` then raises one `feature_failed` for "Fillet 1" with `features` `['f-fil1']`, and a `recomputed` event with `f-fil1` ok removes it. A status with `repairs` `[{kind: 'element', path: '/edges/0', centroid: [0, 0, 20], ...}]` without a `ghost` gives its Problems row the action "Show what 'Fillet 1' lost" and no "Re-pick", and `editFeature` of that feature makes `edges` the active field and renders one `repair-marker`. In that edit, the next edge pick replaces the stored first edge at its place (Edges keeps its length), a second pick of it toggles it as usual, and undoing the pick brings the lost edge back for the next pick to replace.
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
123. U-123. `capturePick` in a Fillet command with a face whose `adjacent` names 4 edges, a fake `listElements` returning their descriptors (short references) and a fake `pickReference` returning each with a full reference (with `nb`): `edges` becomes those 4 full references, in `adjacent` order, after one `pickReference` per edge with its display id. The same click again empties it. With 2 of them already held, it adds the other 2. A `stale_pick` from one `pickReference` adds none.
124. U-124. With `print.orientation` set to a face-down orientation, `runPrintAnalysis` sends it and records it in `reportOrientation`; resetting the orientation makes the badge stale. `exportModel` passes it for 3MF and STL, and not for OBJ and STEP. An `invalid_orientation` `CoreError` clears it and shows a warning notification with the core message.
125. U-125. `COMPONENT_KEYS` has one entry per row of the 3.5 component-key table, with the same key, place and effect (table-driven).
126. U-126. `cameraAfterSketch(current, before, normal)`: with `current` looking along -Z, `before` `DEFAULT_CAMERA` and the normal (0, 0, 1), it returns `before`; with `before` also looking along -Z, it returns a camera that `classifyView` reports as `iso`, with the same target, distance and projection; with `before` null, also `iso`; with `current` 15° away from the normal, it returns null.

Repair mode, fixes, causes and prevention (3.7, 3.9, 3.16, 3.21, 4; the reference stability study section 6.3, U14). `R16-STATUS` is the status of corpus R16 after its edit as engine.md test 115 states it, shortened to two candidates: `f-fil1` `error`, one message `{severity: 'error', code: 'references_not_found', message: "The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet.", field: '/edges', actions: [A0 "Use the front edge of the cut made by 'Extrude 1'" (apply, index 0), A1 "Use the top edge of the cut made by 'Extrude 1'" (apply, index 1), A2 "Pick an edge again" (pick, path `/edges/0`)], detail: 'edges[0] q=edge(f-box1/front|f-box1/top)'}`, and one element repair `{path: '/edges/0', body: 'b-1', description: "the front top edge of 'Box 1'", centroid: [0, -10, 20], ghost: {positions: 0, kind: 'lines', frame: 'world'}, candidates: [C1 (description "the front edge of the cut made by 'Extrude 1'", relation "on the front side of 'Box 1', 5 mm from where it was", meaning true, distance 5, score 0.913978, element edge index 3 of `b-1`'s model-layer mesh), C2 (description "the top edge of the cut made by 'Extrude 1'", relation "on the top of 'Box 1', 5 mm from where it was", score 0.884184, element edge index 4)]}`, with ghost buffer 0 the `f32` values (-10, -10, 20, 10, -10, 20) and revision 7.

127. U-127. With a fake core and `R16-STATUS`, `editFeature('f-fil1')`: the active field is `edges`; the overlay has one `repair-ghost` path with `stroke-dasharray="4 3"` and the title "The front top edge of 'Box 1' (missing)", and no `repair-marker`; two `repair-candidate` markers with `data-number` 1 and 2, placed at the centroids of the `listElements` edge descriptors 3 and 4; the banner holds the buttons "1. The front edge of the cut made by 'Extrude 1'" and "2. The top edge of the cut made by 'Extrude 1'", each with its relation on a second line, then "Pick an edge again" and "Copy details", and no other button whose name starts with "Use "; no text in the inspector or the overlay contains `%`, `0.91` or `0.88`. Hovering button 2 draws edge 4 hovered. With C2's description changed so that no action matches it, button 2 is `aria-disabled`. Esc cancels the command and removes the ghost and the markers. With the ghost absent, one `repair-marker` circle at (0, -10, 20) has that title instead.
128. U-128. In U-127's command, clicking button 1 (or marker 1) calls `applyFix` with A0's `FixRef` (`{feature: 'f-fil1', code: 'references_not_found', field: '/edges', cause: false, index: 0, revision: 7}`); while it runs `data-preview` is `pending` and the candidate buttons are `aria-disabled` with the tooltip "Wait for the preview to update."; when the fake core's events give the record `edges` `[C1's reference]`, the command's `edges` holds it and one `Mod+Z` restores the stored edge. At rest, clicking A0 in the Problems panel calls `applyFix` outside a transaction, and a `stale_fix` `CoreError` shows an error notification with the core's message, `data-code="stale_fix"` and the action "Show problems", and nothing else.
129. U-129. Problems grouped by root with fake statuses in timeline order: `f-dp1` error `reference_not_found`; `f-sk3` and `f-ext3` errors `upstream_failed` with `root` `f-dp1`; `f-sh1` warning `unknown_params`; `f-cha1` `ok` with one info message `reference_followed` and the actions "Keep this match" and "Pick an edge again". `problemRows` gives the top-level rows `f-dp1` (with `f-sk3` and `f-ext3` nested) and `f-sh1`, and one note. The panel shows two `problem-row`s with `data-nested` absent, two with `data-nested="true"` under the first, each nested row with its message and the single action "Show 'Datum plane 1'" (which selects `f-dp1`); `timeline-problems` reads "2 problems"; `problems-notes` reads "Notes" with the count 1 and `aria-expanded="false"`, and opening it shows the note with "Keep this match". The timeline item `f-cha1` has `data-status="ok"`, `data-notes="1"` and the info mark. With only the note, the panel shows "No problems." and the Notes, and the problems button is hidden.
130. U-130. Copy details with a fake clipboard: for the message `{code: 'kernel', message: "Partsmith's geometry kernel could not build this fillet. Fillets fail like this when the edges meet other rounded edges at a corner; try a slightly different radius, or fillet those edges in two steps.", detail: 'fillet: StdFail_NotDone', cause: {text: 'This started after you changed Radius from 1 mm to 3 mm.', actions: []}}`, "Copy details" writes exactly the four lines `kernel`, the message, the cause text and `fillet: StdFail_NotDone`, joined with `\n`, and the polite live region announces "Details copied."; without the cause, the three lines. A message without `detail` has no Copy details button. An `internal_error` `CoreError` with a `detail` shows its error notification with "Copy details". A clipboard that rejects shows the warning notification `details_not_copied`.
131. U-131. A message with the actions A ("Use the top rim of the cut made by 'Extrude 2'", apply) and B ("Pick an edge again", pick) and the cause `{text: "This started after you changed Distance of 'Extrude 2' from 5 mm to 30 mm.", actions: [C ("Set Distance of 'Extrude 2' back to 5 mm", apply, `cause: true`), D ("Undo", undo)]}` renders in this order: the message, A, B, then `cause-line` with the text in `textMuted`, C, D. C calls `applyFix` with its `FixRef` (`cause: true`) and D calls `undo`. A cause action `{label: 'Show the line', run: {kind: 'show', feature: 'f-sk1', entity: 'e-5'}}` of a cause with a ghost (lines (80, 40)-(70, 50) in frame `sketch`) selects `feature:f-sk1` when the solution has no `e-5` (and `sketch:f-sk1:e-5` when it has), sets `AppState.shown`, draws one `cause-ghost` path with the title "Before this change" along the line on the sketch's placement, and frames the selected sketch together with the line; a later recompute that numbers its ghost buffers anew leaves the path where it is, and so does a Show from the banner of an open 'Fillet 1' command, whose implicit cancel's recompute numbers them anew; a selection change removes it.
132. U-132. Loss notifications: before an applied edit `f-fil1` is `ok`; after it `f-fil1` is `warning` with the message `reference_missing` at `/edges/0` whose cause is `{text: "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'.", actions: [Undo (undo), "Show the line" (show)]}`, and the element repair at `/edges/0` with the description "the top edge of 'Extrude 1' along a line of 'Sketch 1'" and two candidates. Exactly one notification appears: `data-severity="warning"`, `data-code="feature_lost"`, the text "'Fillet 1' lost the top edge of 'Extrude 1' along a line of 'Sketch 1'.", the `cause-line` with the cause's text, and the buttons "Show what 'Fillet 1' lost" and "Undo", in that order and no other. With a cause whose first action is the `apply` "Set Count of 'Linear pattern 1' back to 3", the buttons are "Show what 'Fillet 1' lost", "Set Count of 'Linear pattern 1' back to 3" and "Undo". No button names a candidate. "Show what 'Fillet 1' lost" calls `editFeature('f-fil1')` with `edges` active and frames the ghost (`zoomToBounds` with bounds containing both ghost points).
133. U-133. A loss whose key was in the statuses before the edit raises nothing. In a sketch session, two applied sketch edits that each add a `reference_missing` to `f-fil1` raise no notification; `finishSketch` raises one `feature_lost` notification for 'Fillet 1', once. After an `undo` that removes the losses and leaves no feature in error, the notification is dismissed. A feature command that adds a loss raises its notification after OK, compared with the statuses from before the command began, and nothing during its previews.
134. U-134. After an edit, each of `reference_guessed`, `reference_widened`, `region_resized`, `region_remapped` and `sketch_turned_over` (fake statuses, one per case) raises one warning notification "'{label}' needs a check after this change." (`feature_check`) with "Check '{label}'" and "Undo"; a `plane_fallback` with an element repair (description "the top of 'Extrude 1'") on `f-sk2` raises "'Sketch 2' lost the top of 'Extrude 1'." with "Show what 'Sketch 2' lost". `reference_followed` and `region_split` notices (severity info) raise nothing, and neither does a `no_effect` warning whose `root` is the sketch that lost its face.
135. U-135. Drop warning with features `f-ext1`, `f-ext2`, `f-fil1` (a fillet whose `edges` hint has selection `edge`) and a fake `listUses` returning `[{user: 'f-fil1', field: '/edges/0', feature: 'f-ext2'}]`: dragging 'Fillet 1' over the gap between 'Extrude 1' and 'Extrude 2' gives the insertion bar the `warningMark` background classes and shows `drop-hint` "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?"; over its own gap, nothing. Dragging 'Extrude 2' to the end shows "'Fillet 1' uses an edge made by 'Extrude 2'. Drop above 'Fillet 1' instead?". Dropping at a warned gap sends one `move_feature` labeled "Reorder" with no confirmation. `listUses` is called once for the drag's revision; with a `listUses` that has not answered yet, no gap warns, and once it answers, the gap the pointer is over warns without another move.
136. U-136. With uses of `f-ext1` by `f-fil1` and `f-dp1`, the timeline menu of 'Extrude 1' lists "Used by 'Fillet 1', 'Datum plane 1'" (users in timeline order) right after Suppress; choosing it selects both features. With five users, the item reads "Used by 'A', 'B', 'C' and 2 more" for their labels. A feature that nothing uses has no such item, and a use by a feature that is not in the document is ignored.
137. U-137. In a session of `f-sk1` with the uses `{user: 'f-ext1', field: '/profile/sketch', feature: 'f-sk1'}` and `{user: 'f-fil1', field: '/edges/0', feature: 'f-ext1', entity: 'e-3'}`, `refreshUses` runs on entering, one `entity-used` mark with `data-entity="e-3"` and the accessible name "Used by 'Fillet 1'" is shown, and none for `e-1`. Deleting `e-3` commits "Delete" and the sketch hint reads "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it."; deleting `e-1` shows no such hint. For a used point entity `e-p1` used by 'Chamfer 1' through an edge, the hint reads "'Chamfer 1' uses the edge made from this point.".
138. U-138. With `bareNumbers` 4 in an mm document, the units menu has the `keep-sizes` item, `aria-checked="true"`, reading "Keep sizes: add mm to the 4 numbers without a unit"; choosing "in" calls `setDocumentUnits(handle, 'in', true)`. After unchecking it (the menu stays open), choosing "cm" calls `setDocumentUnits(handle, 'cm', false)`. With `bareNumbers` 1 it reads "Keep sizes: add mm to the 1 number without a unit"; with 0 the item is absent and the call sends `false`. The palette command `document.units.in` sends the current `keepSizes`. During a command, the units menu is disabled with `finish_command_first`.
139. U-139. A Fillet command whose preview status has `{code: 'fillet_too_large', field: '/radius', suggestion: '2.3 mm'}` shows `field-limit` "Largest that works: 2.3 mm" under the radius field's `field-error`, in `warningText`, and the field has the `warningMark` border classes instead of `errorMark`. Without `suggestion`, no `field-limit`. In an inch document with radius text `2` and suggestion `2 mm`, no `field-limit`; with radius `2 in` and suggestion `1.9 in`, "Largest that works: 1.9 in". A `shell_too_thick` on `/thickness` shows it under Thickness, and a `chamfer_too_large` on `/distance/expr` under Distance.
140. U-140. With one sketch `f-sk1` whose solution has no region and one finding `{type: 'open_endpoint', entity: 'e-4', point: [0, 0.3], connect: {first: 'e-4.end', second: 'e-1.start', gap: 0.3}}`, `feature.extrude` and `feature.revolve` are disabled with "'Sketch 1' has no closed shape: 2 ends are not connected." and the code `needs_closed_shape`; running it shows an info notification with that text and the action "Show 'Sketch 1'", which enters `f-sk1` and frames (0, 0.3) and (0, 0). With two such sketches of 4 and 2 open ends, the one with 2 is named; with equal counts, the later one. A sketch with no region and no open end (only points) gives "Draw a closed shape in a sketch first.". `outlineProblems` of the first solution is "2 open ends", `openEnds` is 2, and sketch mode draws two `profile-issue` rings.
141. U-141. An Extrude command with op Cut whose status has a `no_effect` warning without actions shows no button named "Reverse" or "Turn on Reverse"; with the action "Turn on Reverse" it shows exactly that one; a `through_all_misses` error behaves the same.
142. U-142. `createSketchOn` with a flat face whose descriptor has `placement` origin (100, 0, 30) and `fallback` origin (0, 0, 30) (a face moved by a Move body) commits a sketch whose `plane` is `{kind: 'face', body, face: <the descriptor's reference>, fallback: <the descriptor's fallback>, fallback_frame: 'body'}`. A `commitSketchChange` in that sketch sends its `plane` exactly as stored, whatever `solution.placement` is.
143. U-143. `selectionValue('axis', ...)` of an edge whose descriptor has `axis_direction` [0, 1, 0] gives `{kind: 'edge', body, edge: <reference>, direction: [0, 1, 0]}`; of a cylindrical face with `axis_direction` [0, 0, 1], `{kind: 'face', body, face: <reference>, direction: [0, 0, 1]}`; of an edge whose descriptor has no `axis_direction`, no `direction` key.
144. U-144. One root, one notification (newcomer error corpus NC11): before an edit every feature is `ok`; after it `f-ext1` is `error` `profile_not_found` with the region repair "the region of 'Sketch 1' around (40, 25)" and a cause, and `f-fil1` is `error` `body_empty` with `root` `f-ext1`. Exactly one notification appears: `data-severity="error"`, "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.", with `features` `['f-ext1', 'f-fil1']` and no `feature_failed` or `features_failed` notification. With a third feature `f-box2` also newly in error with no `root`, a second notification "Box 2 failed. {message}" appears. An `undo` that makes them all `ok` dismisses both. A Delete whose edit raises a loss notification shows no "Deleted {label}." notification.
145. U-145. Fixes during commands and sessions: in an Extrude command, an `apply` action whose events give the record `reverse` true sets the command's `reverse` true and focuses the Distance input; `Mod+Z` sets it back and previews again; an action whose events also change a parameter is undone by `Mod+Z` only in the record's fields, and Cancel reverts both. During the command, an `apply` action in the Problems panel is `aria-disabled` with "Finish or cancel Extrude first.", and a `show` of another feature cancels the command with the `command_canceled` notification. In a sketch session, applying a finding's action "Connect the ends (0.3 mm apart)" calls `applyFix` with the `FixRef` naming the sketch and `/profile_findings/0`, and pushes a `document` entry, so the next `Mod+Z` calls the core's `undo`.
146. U-146. Re-attaching: `f-sk2` has a `plane_fallback` warning with the element repair at `/plane/face` (description "the top of 'Extrude 1'") and two candidates whose actions are "Attach to the top of 'Extrude 3'" and "Attach to the front side of 'Extrude 3'". "Show what 'Sketch 2' lost" runs `reattachSketch('f-sk2')`: `data-context="sketch"`, `sketch-header` `data-state="picking"` with the prompt "Pick a plane or a flat face for 'Sketch 2'", the buttons "1. The top of 'Extrude 3'" and "2. The front side of 'Extrude 3'", and the lost face's `repair-ghost`. Clicking the plane button "Top" commits one `apply_edits` labeled "Attach 'Sketch 2'" whose `update_feature` has `plane` `{kind: 'origin', plane: 'XY'}` and the stored entities, then enters `f-sk2`; picking a flat face instead stores its full reference and its descriptor's `fallback` with `fallback_frame: 'body'`, or, when the fallback of `f-sk2` is in world coordinates, its descriptor's `placement` without `fallback_frame`; the browser row of origin plane XY does what the button Top does, once for two quick clicks; a face whose descriptor's `feature` is 'f-ext2', and the browser row of a datum plane, made after `f-sk2` in the timeline, are refused with `attach_after` naming that feature in the sketch hint, with no edit, and the pick stays open; button 1 calls `applyFix` with the first action's `FixRef`, after which `f-sk2` is entered once its status has no repair, and a `stale_fix` refusal leaves the pick open. Esc returns to Model with no edit. A `pick` action `{feature: 'f-sk2', path: '/plane/face'}` runs `reattachSketch('f-sk2')`.
147. U-147. Wire data: an error response with `detail` "boom" and `actions` `[{label: 'Finish "Fillet" and save', run: {kind: 'finish_transaction'}}]` rejects with a `CoreError` whose `detail` is "boom" and whose `actions` hold that action, and its notification shows no button for it. `decodeEvent` of a `recomputed` whose `f-fil1` status has a repair ghost at buffer 0 and a cause ghost at buffer 1 gives `ghosts` with both as `Float32Array` views of the frame's buffer; a ghost index naming a `u32` buffer throws `CoreFatalError`. A `document_changed` with `bare_numbers` 4 sets `document.bareNumbers` to 4.
148. U-148. `refreshUses` calls `listUses` when `document.uses` is null or older than the revision, and not again at the same revision; a result whose revision is older than the document's when it arrives is dropped; two calls at one revision while the first is in flight send one request.
149. U-149. Repair titles by resolution: with a fake core, `f-fil1` with a `reference_widened` warning whose element repair at `/edges/0` has `resolution` `ambiguous`, description "the back top edge of 'Extrude 1'", no ghost and a centroid: in repair mode of `f-fil1` the `repair-marker` has the title "The back top edge of 'Extrude 1' (check it)"; the same repair with `resolution` `missing` has "The back top edge of 'Extrude 1' (missing)"; no title contains "(missing)" while the resolution is `ambiguous`.

Performance marks (2.18):

150. U-150. `ps:apply-events`: on a store showing a document with handle 1, `applyCoreEvent` of a `document_changed` and then of a `recomputed` of handle 1 records one `ps:apply-events` measure; with `performance.now()` read before the first call (t0), between the calls (t1) and after the second (t2), the measure starts in [t0, t1] and ends in [t1, t2]. None is recorded for a `recomputed` alone, for a `document_changed` followed by a `body_mesh` and then a `recomputed`, or for a `document_changed` of handle 1 followed by a `recomputed` of handle 2. Two `document_changed` events of handle 1 and then its `recomputed` record one measure, which starts after the first call returned. The pair for handle 2, which no action has adopted, records one too.
151. U-151. `ps:expression`: an `ExpressionInput` (unit `length`, starting empty) whose `evaluate` calls return deferred promises. Changing the text to `2` records no `ps:expression` measure while its evaluation is pending, and one once it resolves and the field shows its result. Changing the text to `23` and then to `234` before the `23` evaluation resolves, then resolving both, records one more measure, which starts between `performance.now()` read before and after the change to `234`; none is recorded for `23`. A new document revision that evaluates `234` again records none. Changing the text to `2345` and back to `234` before the `2345` evaluation resolves shows the `234` result at once, and resolving `2345` and the new `234` evaluation records none. Mounting a second field with the text `10 mm` records one once its result shows.
152. U-152. `TIMING_NAMES` holds exactly the eleven names of the 2.18 table, from `ps:usable` to `ps:expression`.

Recovery copies and a core that is still working (2.5, 2.15, 3.19, 4.4):

153. U-153. The recovery copy is the settled recipe (next to U-77): with fake timers and a saved document, a `document_changed` with `transactionOpen` false, `dirty` true and recipe R1 at 0 ms, then `document_changed` events with `transactionOpen` true and recipe R2 (R1 plus a previewed box) at 500, 1,000 and 1,900 ms: exactly one entry is written, at 2,000 ms, and its text is `JSON.stringify(R1, null, 2)`. The commit's `document_changed` (`transactionOpen` false, recipe R3 with the box) at 6,000 ms writes R3 at 8,000 ms. When the command is canceled instead, its `document_changed` (`transactionOpen` false, recipe R1) writes R1 2 s later, and no write ever holds R2.
154. U-154. Recovery timing, with fake timers: settled dirty changes every 1,500 ms from 0 ms to 9,000 ms postpone the 2 s write, but the entry is written at 10,000 ms, holding the recipe of the change at 9,000 ms; a change at 10,500 ms is written at 12,500 ms. A settled `document_changed` with `dirty` false and `localUnsaved` false (an undo back to the saved revision) at 13,000 ms removes the entry at once; a dirty change at 13,500 ms followed by such an undo at 14,000 ms leaves no entry at 16,000 ms.
155. U-155. The IndexedDB recovery store, version 2 (fake IndexedDB): a new database has version 2 and the object stores `recent` and `recovery`, both with key path `id`. Entries `a` (`savedAt` 1000) and `b` (2000) are both kept, writing `a` again replaces only `a`, `list()` returns `[b, a]` and `remove('a')` leaves `[b]`. A record without `text` is skipped by `list` and stays in the store. A version 1 database whose `recovery` store has key path `slot` and holds `{slot: 'current', name: 'bracket', text: '{}', savedAt: 1000, handle: null, reason: 'replaced'}`, next to one `recent` record, is at version 2 after the first call: `recovery` has key path `id` and holds one record equal to the old one without `slot` and with an `id` matching the UUID pattern, and the `recent` record is unchanged. A version 1 `current` record without `text` leaves the new `recovery` store empty.
156. U-156. Running sessions and persistence: two IndexedDB recovery stores over one fake IndexedDB, each with its own page of one fake lock manager (`src/test/fakes`). After page A writes the `autosave` entry `a`, page B's `list()` leaves it out; B lists `a` again after A writes `a` as `replaced`, and after page A ends while it holds `a` (the fake releases a page's locks when it ends); after A removes `a`, it is gone. Writing `a` as `autosave` twice requests one lock. A store with `locks: null`, and one whose lock manager's `query` rejects, lists every entry. With a fake `storage` whose `persisted()` resolves false, the first `write` calls `persist()` once and later writes call neither; with `persisted()` true, `persist()` is never called; a rejecting `persist()` leaves the write done; with `storage: null`, writes work. The memory store lists every entry it holds, newest first.
157. U-157. Replaced, then autosave, then save (the defect of one slot), with a memory recovery store, a fake core and fake timers: document A made dirty is written 2 s later as an `autosave` entry under A's `recoveryId`. Opening document B rewrites that entry as `replaced` with A's settled recipe and shows `stashed`; B gets another `recoveryId`. B made dirty is written 2 s later, and A's entry is unchanged. Saving B removes only B's entry, so `list()` returns A's `replaced` entry alone. In a second run, a dirty change of A at 0 ms and opening B at 1,000 ms write A's entry once, as `replaced`, and nothing is written for A at 2,000 ms or later.
158. U-158. Two sessions at once: apps A and B on two pages over one fake IndexedDB and one fake lock manager, each with its own fake core. A's dirty document writes its entry; B, started afterwards, shows neither `recovered` nor `recovered_more`. B's dirty document writes its own entry under another `recoveryId`, and A's next write replaces only A's, so the store holds two entries, each with its own document's text. A's save removes only A's entry. When page B ends, app C started then shows `recovered` naming B's document, and its Discard removes B's entry, which leaves the store empty.
159. U-159. Startup offers every entry: with entries named "c" (`savedAt` 3000), "b" (2000) and "a" (1000) in a memory store, startup shows one persistent notification, "Partsmith kept unsaved changes to c from {time}, and to 2 other documents." with "Open" and "Discard" (`{time}` as 3.19 formats `savedAt`). Discard removes only c's entry and shows "Partsmith kept unsaved changes to b from {time}, and to 1 other document." Open opens b's text as a document named "b" with `localUnsaved` true and a new `recoveryId`; the store then holds a's entry and b's text under that `recoveryId` (`reason` `autosave`, `savedAt` 2000) and nothing under b's old `id`, and "Partsmith kept unsaved changes to a from {time}." appears. Dismissing it shows nothing more and keeps a's entry. Reopen of a `stashed` entry moves it to a new `recoveryId` the same way, and an Open the core refuses (`open_failed`) keeps the entry it tried to open.
160. U-160. `MutationQueue.discard(error)`: with task 1 started and tasks 2 and 3 waiting, it returns 2 and `pending` goes from 3 to 1; tasks 2 and 3 reject with `error` and their functions are never called; task 1 resolves with its own result; a task run after `discard` starts once task 1 has settled. `changing` is false for an empty queue, true while an `other` task runs and a `change` task waits, true while a `change` task runs, and false while only `other` tasks are started or waiting.
161. U-161. Still working, with fake timers and a fake transport whose responses the test releases: with one waited-on request (`apply_edits`) sent at 0 ms and unanswered, `core.stillWorking` is false at 9,999 ms and true at 10,000 ms; `status-core` then reads "Still working", and the viewport shows `core-working` (`role="alert"`) reading "The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. Stopping it reopens your document as it was after the last change the core finished, and closes any open feature command without keeping it." with the button "Stop and restart core". The response sets `stillWorking` false and removes both. With two waited-on `pick_reference` requests, A sent at 0 ms and answered at 6,000 ms and B sent at 5,000 ms and unanswered, `stillWorking` turns true at 16,000 ms and not before. With the Tauri transport the panel reads "The geometry core has been working for more than 10 seconds, and Partsmith cannot tell whether it will finish. The desktop app cannot stop it yet: wait, or quit Partsmith, which keeps your changes up to the last one the core finished, except those of an open feature command." and has no button.
162. U-162. Stop and restart core, with the worker transport over fake workers: after a rename of the body to "Box" (settled recipe R1, `recoveryId` K), an `apply_edits` renaming it to "Stall" that the worker never answers, and a body visibility change waiting behind it, `stillWorking` turns true after 10 s. Clicking "Stop and restart core" terminates the first worker; the second worker's calls are `feature_catalog` and `list_printer_profiles` (in either order) and then `open_document` with `JSON.stringify(R1, null, 2)`, with no `apply_edits` before it; the document keeps its name, handle and `recoveryId` K, `localUnsaved` is true, `stillWorking` is false, no `core-fatal` appears, and one warning notification reads "Your latest changes were not applied, because the geometry core was stopped before it finished them. Make them again if you still want them." With an Extrude command open whose preview `apply_edits` is the request that never ends, the restart reopens R1 without the extrude, the command is null, the first worker received no `cancel_transaction`, no `command_canceled` appears, and the warning appears. With a stuck `export` of "cube.stl", nothing waiting and no command, the only notification is the warning "cube.stl was not exported, because the geometry core stopped before exporting it. Export it again."

Theme before the bundle runs (2.16.8):

163. U-163. The color-scheme meta: the text of `ui/index.html`, read through Node as U-91 reads `src/index.css` and parsed with `DOMParser` as `text/html`, has exactly one `meta` element whose `name` is `color-scheme`; it is a child of `head`, and its `content` is `light dark`.

Viewport frames (2.18):

164. U-164. `ps:frame`, with fake animation frames 16 ms apart and `performance.now()` standing still: a Shift and middle press, then three pointer moves of an orbit, one per animation frame, record two measures of 16 ms. After one such frame, a release and a hover change rendered in the next animation frame record none, and the wheel zoom that follows records none in its first frame. One orbit frame, an animation frame with nothing to draw, then two more orbit frames record one measure. A left press moved past the gesture threshold, with three frames rendered while the camera stays still, records two measures of 16 ms, and a frame rendered after the release records none.

Recovery copies and a stopped core, continued (2.15, 3.19, 4.4):

165. U-165. A stop says which other work it dropped, with the worker transport over fake workers and cube.ptsm opened from a handle, its body renamed to "Box". With an `export` of "cube.stl" that the worker never answers and a Save waiting behind it, "Stop and restart core" writes no file, and the warnings are exactly "cube.stl was not exported, because the geometry core stopped before exporting it. Export it again." and "cube.ptsm was not saved, because the geometry core stopped before saving it. Save it again.", without `changes_not_applied`. With a rename to "Stall" that the worker never answers and an Open of `two-bodies.ptsm` waiting behind it, the restart leaves "cube" open, and the warnings are exactly `changes_not_applied` and "two-bodies.ptsm was not opened, because the geometry core stopped before opening it. Open it again." With that rename and a New waiting behind it, "cube" stays open, and the warnings are exactly `changes_not_applied` and "The new document was not opened, because the geometry core stopped before opening it. Choose New again."
166. U-166. A restart continues the session it reopens, with a memory recovery store. Over fake workers, after the rename to "Box" (`recoveryId` K) is written as an `autosave` entry, a rename to "Stall" that the worker never answers and a New waiting behind it (which writes K's entry as `replaced`), "Stop and restart core" reopens "cube" with `recoveryId` K, and at once, before any timer runs, the store holds exactly one entry: K, `reason` `autosave`, with the settled recipe's text. A rename to "Lid" then writes K's entry 2 s later with "Lid". In a second run over the fake transport with fake timers, a settled dirty change at 0 ms, a fatal error, a restart whose start fails, and a restart at 3,000 ms that succeeds: the store is empty until that restart, and right after it holds the session's `autosave` entry with the change.
167. U-167. A save keeps the entry of a document an edit left unsaved, with a memory recovery store, fake timers and a file write the test releases: cube.ptsm from a handle, renamed to "Box" and written as an entry; Save starts, and while its file write waits the body is renamed to "Block". Once the write is released and the save ends, `dirty` is true and `localUnsaved` false, the entry is still listed, and 2 s after the second rename it holds "Block".
168. U-168. An opened entry stays when its new copy is not stored: with a recovery store whose `write` resolves false without storing anything, holding entry "b", the startup offer's "Open" opens "b" with `localUnsaved` true, and `list()` still returns b's entry unchanged under its own `id`.
169. U-169. A fatal error, and a restart that fails after a stop, over fake workers. With a rename to "Stall" in flight and a body visibility change waiting when the first worker fails, `core-fatal` shows; "Restart core" reopens the settled recipe with no warning. With the stall rename and the visibility change waiting when "Stop and restart core" is clicked, and a second worker that cannot be created, `core-fatal` shows `core_stopped` and no warning appears; "Restart core" then reopens the document, and `changes_not_applied` appears once.
170. U-170. `upgradeDatabase` from version 2 (fake IndexedDB): a version 2 database whose `recovery` store holds one entry, opened at version 3 with `upgradeDatabase` as its upgrade, still has the stores `recent` and `recovery`, both keyed by `id`, and that entry.

A core that is still working, continued (2.5, 3.21, 4.4):

171. U-171. Only waited-on requests count, with fake timers and a fake transport whose responses the test releases. `BACKGROUND_COMMANDS` is exactly `analyze_print`, `list_elements`, `resolve_references`, `measure`, `evaluate` and `list_uses`. Each of `analyze_print`, `list_elements`, `resolve_references`, `measure`, `evaluate` and `list_uses`, sent alone at 0 ms and never answered, leaves `core.stillWorking` false through 30,000 ms, with no `status-core` and no `core-working`; in Print context with its analysis (3.17) never answered, `health-badge` has `data-state="running"` and reads "Checking" all that time. Each of `apply_edits`, `open_document`, `export`, `pick_reference`, `solve_sketch` and `drag_sketch`, sent alone at 0 ms and never answered, makes it true at 10,000 ms. An `apply_edits` sent at 0 ms and answered at 2,000 ms, then an `analyze_print` sent at 3,000 ms and never answered: false through 60,000 ms. An `analyze_print` sent at 0 ms and never answered, with an `apply_edits` sent behind it at 5,000 ms: false at 14,999 ms and true at 15,000 ms. An `analyze_print` sent at 0 ms and answered at 8,000 ms, with an `apply_edits` sent at 1,000 ms and never answered: true at 18,000 ms and not before. An `evaluate` sent at 0 ms and never answered, with an `app.waitOn` of it started at 5,000 ms: false at 14,999 ms and true at 15,000 ms. An `app.waitOn` started at 0 ms of an `evaluate` answered at 2,000 ms: false through 30,000 ms. With Fillet open on `cube.ptsm` and `list_elements` never answered, the top face picked into Edges (`capturePick`) sends `list_elements` and makes it true 10,000 ms after the pick and not before. In sketch mode on `f-sk1` with the line tool, a line started and "25" typed into Length (its `evaluate` never answered) at 0 ms, and Enter (`commitDrawInputs`) at 5,000 ms: false at 14,999 ms and true at 15,000 ms.
172. U-172. The panel is drawn over the model, with the app rendered (`App`) over a fake core and `cube.ptsm` open in Print context. With `core.stillWorking` true, `core-working` is inside `viewport`, in a `pointer-events-none` layer with `z-30` where only the panel's card is `pointer-events-auto`, a child of the free-area layer (the last child of `viewport`), which has `isolate`; `viewport` still contains the canvas element it had before, `view-cube` and `viewport-controls`; the toolbar "Print tools" has the buttons "Overhang map" and "Lay on face", and the radiogroup "Context" has "Print" checked. "Overhang map" has `aria-pressed` true, clicking it sets it to false, and choosing the radio "Model" sets `data-context="model"` and shows the toolbar "Model tools" while `core-working` is still shown. With `stillWorking` false again, `core-working` is gone and the canvas is still that same element.

Tooltips after Esc (3.24):

173. U-173. Esc opens no tooltip, with the popover stand-in. A `Tooltip` labeled "Export" around a button: after a keydown of Escape on the document, focusing the button leaves the tooltip closed; after a keydown of Tab, blurring and focusing the button opens the tooltip at once. A `Popover` whose trigger button is wrapped in a `Tooltip`, opened by clicking the trigger: Escape closes the popover and returns focus to the trigger, whose tooltip stays closed.

Islands and the free area (3.2, 2.11, 3.14):

174. U-174. Islands, rendering `App` with fakes, `cube.ptsm` open, the Parameters panel open, the timeline expanded and the body row "Cube" selected (so the selection inspector shows): `top-bar`, `browser`, `side-panel` and `timeline` each have the classes `relative`, `z-10`, `rounded-xl`, `border` and `shadow-md` and `COLOR_CLASSES.surface.background`, `COLOR_CLASSES.forcedEdge.border` and `COLOR_CLASSES.elevation.shadow`; `status-bar` has the same with `rounded-lg` in place of `rounded-xl`; collapsing the timeline gives it `rounded-lg`. `browser` is `flex flex-col overflow-hidden` without `overflow-y-auto`, and the body around its tree is `min-h-0 flex-1 overflow-y-auto`. The app root has `p-2`, `@container/app` and `COLOR_CLASSES.stage.background` and no `gap-` class; the top bar has `mb-2`, the browser `mr-2`, the side panel `ml-2`, the timeline and the status bar `mt-2`. `viewport` has none of the classes `relative`, `absolute` and `fixed`; the element holding the canvas is `absolute inset-0` and has the class `peer`; `view-cube`, `viewport-controls` and `inspector` lie inside the free-area layer, the last child of `viewport`, with `pointer-events-none relative isolate size-full rounded-xl`, the classes `peer-focus-visible:outline-2` and `peer-focus-visible:-outline-offset-2` and `COLOR_CLASSES.accent.outline`. The notifications region follows the status bar's region and has `relative z-30 col-start-2 row-start-2`. With `rect-sketch.ptsm` open and "Sketch 1" edited, `sketch-overlay` lies outside the free-area layer, in a `pointer-events-none absolute inset-0` element of `viewport`, and `sketch-header` inside it.
175. U-175. Docking by the window's width in rem (`window.innerWidth` and the root font size stubbed): with an open browser and the Problems panel, at 1000 CSS pixels and a 16 pixel root font (62.5rem) a pointer press on `status-bar` closes both, and Esc closes a reopened browser; at 1030 pixels (64.375rem, a content box of 63.375rem inside `p-2`, where the panels float) a press closes both too; at 1100 pixels (68.75rem) neither a press on `status-bar` nor Esc closes them; at 1280 pixels and a 32 pixel root font (40rem) a press closes both, as at 1000. `createAppStore` starts with the browser closed for `window.innerWidth` 1000 and 1030 with a stored `uiScale` of 1 and for 1280 with a stored `uiScale` of 2, and open for 1040 and 1280 with `uiScale` 1. The browser has the classes `@max-5xl/app:absolute`, `@max-5xl/app:inset-y-0`, `@max-5xl/app:z-20`, `@max-5xl/app:shadow-lg`, `@max-5xl/app:col-start-2`, `@max-5xl/app:col-end-3`, `@max-5xl/app:row-end-3`, `@max-5xl/app:left-0` and `max-w-full`; the side panel the same with `@max-5xl/app:right-0`. `src/test/setup.ts` sets `window.innerWidth` to 1280 for every test, since jsdom's default 1024 is narrower than 65rem and would start every rendered `App` with the browser closed.
176. U-176. `windowProjection`, for the iso view with target (0, 0, 0), distance 200, both projections, near 1, far 1000, and the view `{width: 1200, height: 800, free: {x: 280, y: 64, width: 640, height: 600}}`: the target projects to canvas pixel (600, 364); the target plus `up(camera)` times half of `visibleHeightAtTarget` projects to y 64, and the target plus `right(camera)` times half of `visibleHeightAtTarget` times 640/600 projects to x 920 (each within 0.001 pixel, since the matrices are 32-bit floats); a canvas point taken to free-area coordinates (2.11) and back is unchanged; with `free` equal to the whole canvas the matrix equals `projectionMatrix(camera, 1.5, 1, 1000)` element by element, and so does a free area 0 pixels wide. With the XY sketch placement and the same camera and view, `sketchToPixel` of the sketch point (30, -20) is the canvas pixel that `windowProjection` gives for the world point (30, -20, 0), and `pixelToSketch` of that pixel gives (30, -20) back (each within 1e-6).
177. U-177. Framing in the free area, with `FakeBackend`, the shared `ResizeObserver` stand-in of `src/test/fakes` (`stubResizeObserver`; jsdom has no `ResizeObserver`), a canvas of 1200 by 800 CSS pixels and the free-area element stubbed at (280, 64, 640, 600) relative to it: every frame's `projection` equals `windowProjection(camera, view, near, far)` for that view; `fit()` of the box (-10, -10, 0) to (10, 10, 20) in the front view, orthographic, draws the box from x 360 to 840 and y 124 to 604 (80 percent of the free area's height, around its center (600, 364), within 0.5 pixel) and sets `data-camera-target` "0,0,10". Stubbing the free area at (0, 64, 920, 600) (the browser closed) and firing the observer renders a frame that draws the box from x 220 to 700 at the same y, with the camera unchanged; at (0, 64, 920, 656) (the timeline collapsed) from y 129.6 to 654.4 around 392. `pan` by 100 pixels moves the target by 100 × 25 / 656 millimeters, the grid spacing follows the free area's scale (`drawnGridSpacing` with height 656), and a later change of the free area keeps that camera. After `fit()` of the box (-100, -10, 0) to (100, 10, 40) in the front view with the free area at (280, 64, 640, 600), stubbing it at (280, 64, 400, 600) frames the box again, from x 320 to 640. With a canvas of 1280 by 720, the free area at (280, 64, 992, 548) and the inspector at (74, 16, 320, 400) in it, `fit()` of the first box puts the box's center at canvas pixel (973, 338) and draws it 438.4 pixels tall; moving or closing the inspector then leaves the camera unchanged.
178. U-178. Hover ends at an island: after a pointer move over a face (the `FakeBackend` pick answers a face), `data-hover-kind` is `face`; a `pointerleave` on the canvas sets it to `none`, and a wheel or click event dispatched on `top-bar` changes neither the camera nor the selection.
179. U-179. Axis roles: `AXIS_ROLES` is `{X: 'axisX', Y: 'axisY', Z: 'axisZ'}`; `COLOR_CLASSES.axisX` is `{foreground: 'text-rose-600 dark:text-rose-400', border: 'border-rose-600 dark:border-rose-400'}`, `axisY` the same with `emerald-700` and `lime-400`, `axisZ` with `sky-700` and `sky-300` (also rows of U-90's table, which `roles.test.ts` holds as data, as it holds U-70's, so the new rows of 2.16.3 and 2.16.6 are added there); `VIEWPORT_STEPS` of `axisX`, `axisY` and `axisZ` name the same steps as the interface roles; U-70's rows include the axis rows of 2.16.6.
180. U-180. Axis fields: `buildInspectorModel` gives Move body's `translation[0]` to `[2]` the axes X, Y and Z with `world` true; a box's `position[1]` with `plane` `{kind: 'origin', plane: 'XY'}` axis Y with `world` true, with `plane` the origin XZ plane or a datum plane `world` false; the extrude's distance, a box's size fields and a length field labeled "Offset X" at a path without an index have `axis` null.
181. U-181. Axis letters in fields, the Inspector with a fake catalog: in the Move body command each input named "Move X", "Move Y" and "Move Z" has `pl-6`, and its field holds an `axis-letter` reading X, Y and Z with `data-axis` the letter, `data-world="true"`, `aria-hidden="true"`, `font-semibold` and `COLOR_CLASSES.axisX.foreground`, `axisY` and `axisZ`; in a Box command on the origin XZ plane the Position letters have `data-world="false"` and `COLOR_CLASSES.textMuted.foreground`; the inputs' accessible names stay "Move X" and "Position X"; an expression field without an axis has no `axis-letter` and no `pl-6`.
182. U-182. Coordinate readouts, BodySection and ElementsSection with fakes: `body-size` of the bounds (0, 0, 0) to (40, 20, 10) has `data-value` "40 × 20 × 10 mm" and the `textContent` "X 40 Y 20 Z 10 mm" (single spaces, from the space text nodes of 3.8); `body-center` of that box "X 20 Y 10 Z 5 mm" with `data-value` "20, 10, 5 mm"; each is `flex w-full flex-wrap items-baseline gap-x-2 gap-y-1`, follows its label (`text-xs`, `COLOR_CLASSES.textMuted.foreground`) on a line of its own, and holds three cells `flex flex-1 basis-0 items-baseline gap-1`, each with an `axis-letter` (X, Y and Z, without `aria-hidden`, in `font-semibold` and their axis roles' foreground) and a number with `tabular-nums` and without `font-mono`, then the unit in a `shrink-0` span; a body without geometry has `body-size` "Empty" with `data-value` "Empty", no `axis-letter` and no `body-center`; one vertex at (10, -10, 20) shows `vertex-position` "X 10 Y -10 Z 20 mm" with `data-value` "10, -10, 20 mm".
183. U-183. Axis colors in the scene, with `FakeBackend`, in both themes: with the origin axes shown, their draw is dashed, 1.25 CSS pixels wide, and its `elementColors` are the linearized `axisX`, `axisY` and `axisZ` colors of the theme for the X, Y and Z elements, and a hovered origin axis is drawn 2.5 CSS pixels wide in `highlight`; with the three origin planes shown, the XY, XZ and YZ planes' fill draws have the colors of `axisZ`, `axisY` and `axisX` at opacity 0.15 and their outline draws at 0.6, a hovered plane's outline is 2.5 CSS pixels wide, and a datum plane's draws keep `datum`. In sketch mode the draw of the sketch's axes has `axisX` and `axisY` on an XY sketch, `axisX` and `axisZ` on an XZ sketch, `axisY` and `axisZ` on a YZ sketch, and `sketchReference` for both on a datum sketch; the overlay of an XZ sketch holds the `axis-label`s X and Z, and that of a datum sketch none. The axis labels of the overlay are `axis-label`s with `data-axis`, `font-semibold` and their axis role's foreground on `surface`.
184. U-184. View cube edges: each of the 6 faces holds two `aria-hidden` `pointer-events-none` overlays, one `absolute inset-0 border-y-2` and one `absolute inset-0 border-x-2`, with the `border` facets of `axisX` and `axisY` on Top and Bottom, `axisX` and `axisZ` on Front and Back, `axisY` and `axisZ` on Left and Right; no face carries a `border-zinc-` class or `COLOR_CLASSES.forcedEdge.border`; the 26 view buttons and "Home view" keep their names, and "Home view" is `rounded-md border shadow-md` with `COLOR_CLASSES.surface.background`, `COLOR_CLASSES.forcedEdge.border`, `COLOR_CLASSES.elevation.shadow` and `COLOR_CLASSES.hover.opaque`, and `absolute -top-7 -left-7 size-6`, outside the cube's box; each face element has `COLOR_CLASSES.surface.background`, and while no target is hovered none of its cells has a `bg-` class.
185. U-185. Browser origin rows: the icons of the rows "X axis", "Y axis" and "Z axis" have the `axisX`, `axisY` and `axisZ` foreground classes, and those of "Top plane (XY)", "Front plane (XZ)", "Right plane (YZ)" and "Origin" none of the axis roles' classes.
186. U-186. Corners and slot (class checks): `view-cube` is `pointer-events-auto absolute top-11 right-9 size-24`; `viewport-controls` is `pointer-events-auto flex flex-col gap-1 rounded-xl border p-1 shadow-md` with `surface`, `forcedEdge` and `elevation`, the first child of an element with `pointer-events-none absolute inset-y-4 left-4 flex flex-col flex-wrap content-start items-start gap-2`, whose last child in Print with the overhang map on is `overhang-legend`, with `mt-auto rounded-xl border shadow-md`; `EmptyState`, `SketchHeader` and `PrintHint` render `TopCenterSlot`: a slot `pointer-events-none absolute inset-4 flex flex-col @container/slot` holding an `aria-hidden` spacer `@max-xl/slot:h-44` and then `top-center-slot`, `flex shrink-0 flex-col items-center gap-2 @xl/slot:mx-38`; `empty-state` is `rounded-xl` with `shadow-md`, and `sketch-header`, `sketch-hint` and `print-hint` are `rounded-lg border shadow-md`.
187. U-187. Inspector bounds, with the free-area layer's rectangle stubbed at (0, 0, 992, 548), `viewport-controls` at (16, 16, 42, 150) in it and the inspector's size at 320 by 300: an inspector opened with nothing preselected is placed at (74, 16), right of the controls; with an anchor at (600, 200, 40, 40) in canvas pixels and the free area at (280, 64) in the canvas, at (376, 136) (the anchor moved by the free area's offset, then `placeInspector`'s rule). With `top-center-slot` at (176, 16, 640, 80) (a sketch header and its hint) and nothing preselected, at (74, 112), under the slot's content; the same with an inspector 450 pixels tall, which does not fit under it, at (74, 16). In a free-area layer 384 by 548 with the same controls and panel, where right of the controls leaves 326 pixels, less than the panel's 320 plus two gaps, at (16, 16). Each of these is checked twice: on `placeInspector` and `inspectorBounds`, and on the `Inspector` rendered in a free-area layer whose rectangles are stubbed (the layer at (280, 64, 992, 548) on the page, the card's `offsetWidth` and `offsetHeight` at 320 and 300) with a fake viewport whose `viewWindow()` has that free area and whose `screenRectangle()` is (600, 200, 40, 40), by the card's translate. With the view cube's reach at (832, 16, 124, 124) in the layer (its box at (860, 44, 96, 96) and its Home view button at (832, 16, 24, 24)), items at (520, 136, 40, 40) in the free area, right of which the card would end at 912, open it left of them at (184, 136); in a free area 384 wide the cube moves no side.
188. U-188. Shapes and depth of the shared components: `Button` and `IconButton` are `rounded-md` and never `rounded-full`; `Menu` and `Popover` are `pointer-events-auto rounded-lg border shadow-lg` with `COLOR_CLASSES.surfaceRaised.background`, `COLOR_CLASSES.forcedEdge.border` and `COLOR_CLASSES.elevation.shadow`, `Tooltip` `rounded-lg border shadow-lg` with the same three, a menu item `rounded-md` and a menu separator `COLOR_CLASSES.divider.border`; the palette's dialog is `rounded-xl border shadow-lg` with `COLOR_CLASSES.surfaceRaised.background`, `COLOR_CLASSES.forcedEdge.border` and `COLOR_CLASSES.elevation.shadow`, the header row holding its search field `border-b` with `COLOR_CLASSES.divider.border`, and `palette-footer` without a `border-t` class; the inspector and a notification are `rounded-xl border shadow-lg` with `COLOR_CLASSES.forcedEdge.border`, the inspector with `COLOR_CLASSES.surface.background` and a notification with `COLOR_CLASSES.surfaceRaised.background`, and every rendered element with a `shadow-xs`, `shadow-md` or `shadow-lg` class (the frame islands, the inspector, a notification, the palette, the viewport controls, a checked segment) carries `COLOR_CLASSES.elevation.shadow`; the command inspector's last child is its footer, `shrink-0` without a `border-t` class or `COLOR_CLASSES.divider.border`, after its body `min-h-0 flex-1 overflow-y-auto border-y` with `COLOR_CLASSES.forcedEdge.border`, `COLOR_CLASSES.divider.overflowTop` and `COLOR_CLASSES.divider.overflowBottom`, which holds the fields; `expression-suggestions` is `rounded-lg border shadow-lg` with `COLOR_CLASSES.surfaceRaised.background` and `COLOR_CLASSES.forcedEdge.border`; `inspector-error` is `rounded-lg`; a `draw-input` is `rounded-md` without a `shadow-` class; the status bar's buttons and the collapsed timeline's buttons carry `COLOR_CLASSES.focusRing.inset` and not `COLOR_CLASSES.focusRing.outline`; no file under `src/` contains `shadow-sm`, and `shadow-xs` appears only in `src/components/SegmentedControl.tsx`.
189. U-189. Overlay elements stay in the free area, with `FakeBackend`, a canvas of 1200 by 800 CSS pixels and the free area at (280, 64, 640, 600): a constraint glyph whose anchor projects to canvas x 100 is `hidden`, and one whose anchor projects to x 600 is positioned there; with the draw inputs' `offsetWidth` and `offsetHeight` stubbed at 168 and 24, draw inputs whose rubber band's midpoint projects to canvas x 915 have their right edge at or left of x 912 (0.5rem inside the free area's right edge at 920) and stay visible.
190. U-190. Emphasis at every UI scale (`devicePixelSizes`, `src/viewport/webgl2/layout.ts`): a draw's `size` and `pickSize`, CSS pixels that already carry the UI scale, are multiplied by the pixel ratio only (2.5 and 14 at UI scale 2 and ratio 1; 5 at ratio 2); the emphasized line width (2.5), point size (7), dash (6) and dash period (10) by the UI scale and the pixel ratio (15, 42, 36 and 60 at scale 3 and ratio 2); at UI scales 1, 1.25, 1.5, 2 and 3 the emphasized width exceeds the resting width of an origin axis (1.25 times the scale) and of a plane outline (1 times the scale).
191. U-191. The inspector stays in the free area, with the rectangles of U-187: a kept dragged position (900, 400) opens the card at (672, 248), and the kept position stays (900, 400); with the card at (672, 16), a layer that shrinks to 624 pixels moves it to (304, 16) when the `ResizeObserver` stand-in fires.
192. U-192. Framing beside an inspector under an island, with `FakeBackend`, a canvas of 1280 by 720, the free area at (280, 64, 624, 548), the front view and the box (-10, -10, 0) to (10, 10, 20): with the inspector's box entirely under the side panel (page x 952 to 1272) or under the browser (page x -120 to 200), `fit` centers the box at canvas x 592; with the box from 500 to 820 in the free area, at 530, the middle of the part left of it.
193. U-193. Crashed regions (`RegionBoundary`): a region renders its children until one throws; then a panel's fallback carries its `fallbackClassName` and `flex flex-col overflow-hidden`, its body `min-h-0 flex-1 flex-col overflow-y-auto p-4` holds the message, and "Reload" is `h-8`; a bar's (`bar`) body is `min-h-0 flex-1 items-center overflow-hidden px-3` without `p-4`, its message `min-w-0 truncate`, and "Reload" `h-6 text-xs`. `Button` is `h-8 px-3 text-sm`, and `size="compact"` makes it `h-6 px-2 text-xs` without `h-8`.
194. U-194. A narrow top bar: `document-name` is `min-w-0 truncate` in a `min-w-20` wrapper; the texts "Search" of `palette-button` and "Export" of the Export button are `@max-3xl:sr-only`, the accessible name of `palette-button` still starts with "Search" and the Export button's is "Export", the Export button has `@max-3xl:px-2`, and hovering `palette-button` shows the tooltip "Search".
195. U-195. The palette in the window: its layer is `fixed inset-x-4 top-1/6 bottom-4 items-start`, the dialog `max-h-full flex flex-col overflow-hidden` and its listbox `max-h-96 min-h-0 flex-1 overflow-y-auto`; with the body option "Cube" active, which has no description, `palette-footer` reads exactly "Enter to run, Esc to close", and for "zzz" exactly "Esc to close".
196. U-196. Small layout rules: an empty `expression-result` has no child and the class `empty:hidden`; the Parameters panel's header is `h-12 px-4`, without a `border-b` class or `COLOR_CLASSES.divider.border`; the Sketch toolbar has no `hr` while a plane is picked and one while a sketch is edited; the three plane buttons of `sketch-header` are the children of one `flex gap-2` child of it; the empty-state legend's hyphenated words ("Shift+middle-drag,", "right-drag", "middle-drag", "Shift+right-drag.") are `whitespace-nowrap` spans; a choice shown as a select is `appearance-none pr-8` and followed by an `aria-hidden` `svg` that is `pointer-events-none absolute right-2 size-4`; `top-bar`, `timeline` and `status-bar` carry `data-bar`, `browser` and `side-panel` do not.
197. U-197. Floating elements of a bar (`placeNextTo`): in an 800 by 600 window, a 120 by 60 element under an anchor at (100, 16, 40, 32) inside a `data-bar` element at (8, 8, 784, 48) is placed at (100, 60), 0.25rem under the bar; with the bar at (8, 564, 784, 28) and the anchor at (100, 566, 40, 24), at (100, 500), above it. A submenu's item is not a control of the bar: with the bar at (8, 8, 784, 48), an element with `popover` inside it and an anchor at (104, 260, 192, 32) inside that, the element placed `right` of the anchor is at (300, 260), level with the item.
198. U-198. Face states tinted in sRGB: the faces' fragment shader defines `mixEncoded` as `decodeSrgb(mix(encodeSrgb(linear), encodeSrgb(target), amount))` and tints the problem, selected and hovered states with `mixEncoded(color, u_problem, 0.5)`, `mixEncoded(color, u_highlight, 0.35)` and `mixEncoded(color, u_highlight, 0.2)`, never `mix(color, u_highlight`.
199. U-199. What the inspector opens beside (`elementExtentPoints`), on the cube's `boxMesh`: the front face's points span (-10, -10, 0) to (10, -10, 20); a face in the back plane facing +Y spans y 10; a face in the front plane facing +Y, which no triangle does, gives its centroid; the top front edge its two ends (-10, -10, 20) and (10, -10, 20); without a mesh, a curved face and a vertex give their centroid. `screenRectangle` of the selected front face in the front view at 20 pixels per mm (canvas 1000 by 500) is 400 by 400 pixels from x 300.
200. U-200. Framing an entered sketch: the cube's sketch solved 30 mm to the right, whose constraints place a corner by distances from `origin`, frames (0, -10, 0) to (40, 10, 0) with margin 0.1; with only its coincident and horizontal constraints, (20, -10, 0) to (40, 10, 0).
201. U-201. Sketch axis labels beside their axes (`besideAxis`, 10 pixels per millimeter, the area (20, 30) to (180, 170)): 12 pixels below the X axis from (180, 100) to (180, 112), left of the Y axis from (100, 30) to (88, 30), clamped from (180, 165) to (180, 170), and unchanged while the negative side is behind the eye; in the sketch overlay of a 1024-pixel view the X and Y labels of an XY sketch are at (1008, 524) and (500, 16), and the Z label of an XZ sketch at (500, 16).

Type and self-hosted fonts (2.16.1, 2.16.7, ADR 0031). In jsdom, which applies no stylesheet, an element has tabular figures when it or an ancestor carries `tabular-nums`, and proportional figures when neither does; it is code when it or an ancestor carries `font-mono`.

202. U-202. Font files, read through Node as U-91 reads `src/index.css`: `ui/public/fonts` holds exactly the four files of 2.16.7's table, each with the stated byte length and SHA-256 (`node:crypto`); each `.woff2` file starts with the four bytes `wOF2`; `Geist-OFL.txt` starts with "Copyright (c) 2023 Vercel, in collaboration with basement.studio" and `JetBrainsMono-OFL.txt` with "Copyright 2020 The JetBrains Mono Project Authors", and both contain "SIL OPEN FONT LICENSE Version 1.1". Each `url(...)` in `src/index.css` is `/fonts/` followed by the name of one of the two `.woff2` files, and each of them is named once.
203. U-203. The preload: `ui/index.html`, read and parsed as in U-163, has exactly one `link` element whose `rel` is `preload`; it is a child of `head`, its `href` is `/fonts/Geist-Variable.woff2` (the `url(...)` of the `@font-face` whose `font-family` is `'Geist'` in `src/index.css`), its `as` is `font`, its `type` is `font/woff2`, and it has a `crossorigin` attribute whose value is empty or `anonymous`. `index.html` has no `link` whose `rel` is `stylesheet`, and neither it nor `src/index.css` contains `fonts.googleapis.com` or `fonts.gstatic.com`.
204. U-204. Type rules in source: no `.ts` or `.tsx` file under `src/` other than those of `src/generated` and the `*.test.ts(x)` files (`src/index.css` names the families and their features, 2.16.1) contains, case-sensitively, `FONT_CLASSES`, `font-sans`, `font-[`, `font-feature`, `tnum` or `slashed-zero`, or a `kbd`, `samp` or `pre` JSX element (`<kbd`, `<samp` or `<pre` followed by `>` or whitespace).
205. U-205. Expression fields: an `ExpressionInput` (unit `length`, a fake `evaluate`, parameter `size` = `20 mm`) has its input in tabular figures and not code, and without `[font-variant-ligatures:no-contextual]`; typing `10` shows `expression-result` "mm" in tabular figures and not code, and typing `si` opens `expression-suggestions`, whose options are in tabular figures and not code. With `code`, the input is code with `[font-variant-ligatures:no-contextual]` and carries no `tabular-nums`, `expression-result` is in tabular figures and not code, and the options are code. The command inspector's Distance input (U-31's extrude) and `entity-size` (U-115's line) are not code and are in tabular figures. A `NumberField` of an `integer` param holding 12 has its input in tabular figures and not code, and a `TextValueField` has neither.
206. U-206. Code surfaces, with fakes and the parameters `size` = `20 mm` (comment "Wall") and `gap` = `size / 4` (recomputed value 5 mm): in the Parameters panel the inputs "Name of size" and "Expression of size" are code and carry `[font-variant-ligatures:no-contextual]`, "Comment of size" is neither, and `parameter-value` is in tabular figures and not code. In the browser, the row of `size` holds the text "size = 20 mm" in a code element without `[font-variant-ligatures:no-contextual]`, and the row of `gap` holds "gap = size / 4" the same way and its value "5 mm" in an element in tabular figures that is not code. In the palette, the option of `size` holds "size = 20 mm" in a code element, its detail "Wall" and its kind tag "Parameter" are not code, and the option "Extrude" has no code element. The inspector's unsupported control (2.12) shows its JSON in a `code` element that carries `font-mono`.
207. U-207. Tabular and proportional surfaces, rendering `App` with fakes. With `cube.ptsm`, a fake status that puts "Extrude 1" in error and the timeline collapsed: `status-recompute`, `status-grid`, `health-badge`, `timeline-problems` and the collapsed timeline's "2 features" are in tabular figures. With the body row "Cube" selected: each number of `body-size` and `body-center` and the value of the `measure-row` "Volume" are in tabular figures, and `selection-title` is proportional; with three edges selected, `selection-title` "3 edges" is proportional. In sketch mode on `rect-sketch.ptsm`: each `sketch-dimension` and `status-dof` are in tabular figures and `sketch-header` is proportional; with the line tool active and a line started, each `draw-input` is in tabular figures. In Print with a report of two warnings: the "Print check" button's count and both labels of `overhang-legend` are in tabular figures, and the legend's caption is proportional. Every `notification`, `field-error` and `cause-line` is proportional.
208. U-208. No stray code face, rendering `App` with fakes: with `cube.ptsm` (no parameters), the body row "Cube" selected, the timeline expanded, the Problems panel open and the palette open with an empty query, no element carries `font-mono` or `[font-variant-ligatures:no-contextual]`; with `rect-sketch.ptsm` in sketch mode and the line tool active, none does either.

Flat islands (2.16.3, 3.2, 3.23, 3.24, ADR 0032):

209. U-209. No outlines, rendering `App` with fakes: with `cube.ptsm` open, the Parameters panel open, the timeline expanded, the body row "Cube" selected and an info notification shown, then in turn with the palette open and with the Document menu open, every element with a `shadow-md` or `shadow-lg` class outside a closed popover (a tooltip at rest) has the classes `border` and `COLOR_CLASSES.forcedEdge.border`, and `COLOR_CLASSES.surfaceRaised.background` if it is `document-menu`, the palette's dialog or a notification and `COLOR_CLASSES.surface.background` otherwise; no element has a `border-zinc-` class other than `borderControl`'s `border-zinc-500` and `divider`'s, and `divider`'s only on menu separators (`hr` children of `document-menu`) and on the palette's header row; `PanelHeader`'s element, every `parameter-row`, the command inspector's footer and `palette-footer` have no `border-t` or `border-b` class; and every `hr` of the Model toolbar is `mx-1 h-6 w-0 shrink-0 border-0` without `border-l` or `COLOR_CLASSES.divider.border`. With `rect-sketch.ptsm` and "Sketch 1" edited, `sketch-header`'s status element has no `border-l` class, and every `constraint-glyph` and `sketch-dimension` that is not selected has `border` and `COLOR_CLASSES.forcedEdge.border`, and a selected one `border` and `COLOR_CLASSES.accent.border`. In the first setup the browser and `side-panel` have `COLOR_CLASSES.surfaceRaised.overlay` beside `COLOR_CLASSES.surface.background`, with `@max-5xl/app:z-20` and `@max-5xl/app:shadow-lg`, and `top-bar`, `timeline`, `status-bar` and `inspector` do not. With the Keyboard and mouse panel open, none of its sections has a `border-t` class; with the print check panel open (`overhang.ptsm` in Print), `health-list`'s first child has no `border-b` class.
210. U-210. Segmented control, `SegmentedControl` with options A, B and C and value B: the radiogroup has `h-8`, `rounded-md`, `border`, `p-0.5`, `COLOR_CLASSES.tonal.background` and `COLOR_CLASSES.forcedEdge.border`, and not `COLOR_CLASSES.field.background`; every label has `COLOR_CLASSES.forcedEdge.outline`; B's label has `rounded-sm`, `COLOR_CLASSES.thumb.background`, `shadow-xs`, `COLOR_CLASSES.elevation.shadow`, `COLOR_CLASSES.text.foreground`, `font-semibold`, `outline` and `-outline-offset-1`, and not `COLOR_CLASSES.selection.background`; A's and C's labels have `COLOR_CLASSES.textMuted.foreground` and `COLOR_CLASSES.hover.foreground` and no `outline`, `-outline-offset-1`, `bg-` or `shadow-` class; choosing C moves B's classes to C and A's to B; every radio keeps `COLOR_CLASSES.focusRing.outline`. With `disabled`, B keeps `COLOR_CLASSES.thumb.background`, `shadow-xs`, `font-semibold`, `outline` and `-outline-offset-1` and has `COLOR_CLASSES.textMuted.foreground`, A and C have `COLOR_CLASSES.textDisabled.foreground`, and no label has `COLOR_CLASSES.hover.foreground`. The top bar's context switcher, an inspector choice field and the section popover's Plane are this component.
211. U-211. Buttons and control boundaries: `Button` without `variant` has `border`, `COLOR_CLASSES.forcedEdge.border`, `COLOR_CLASSES.tonal.background`, `COLOR_CLASSES.hover.background` and `COLOR_CLASSES.text.foreground`; with `aria-disabled`, `COLOR_CLASSES.tonal.background` and `COLOR_CLASSES.textDisabled.foreground` without `COLOR_CLASSES.hover.background`; with `aria-pressed` false, `COLOR_CLASSES.forcedEdge.outline` and not `COLOR_CLASSES.accent.outline`, and with `aria-pressed` true, besides, `COLOR_CLASSES.selection.background`, `COLOR_CLASSES.text.foreground`, `outline` and `-outline-offset-1` and neither `COLOR_CLASSES.tonal.background` nor `COLOR_CLASSES.hover.background`; with `variant="banner"`, `COLOR_CLASSES.tonal.banner` and `COLOR_CLASSES.hover.banner` in place of `COLOR_CLASSES.tonal.background` and `COLOR_CLASSES.hover.background`; with `variant="primary"`, `COLOR_CLASSES.accent.border`, `COLOR_CLASSES.accent.background`, `COLOR_CLASSES.accent.foreground` and `COLOR_CLASSES.accentHover.background` and no `tonal` class. A Fillet command whose preview status is that of U-139 with the core's action "Use 2.3 mm" (an `apply` action, 4.2) shows that fix in `inspector-error` as a `Button` with `COLOR_CLASSES.tonal.banner` and `COLOR_CLASSES.hover.banner`. In `TopBar`, `palette-button` and the Export button have `COLOR_CLASSES.hover.background`, neither `border` nor `COLOR_CLASSES.forcedEdge.border`, and no background class at rest (neither `COLOR_CLASSES.field.background` nor `COLOR_CLASSES.tonal.background`), `palette-button` with `COLOR_CLASSES.textMuted.foreground` and Export with `COLOR_CLASSES.text.foreground`; in the Sketch toolbar, "Finish sketch" has `border` and `COLOR_CLASSES.accent.border`, and in the Print toolbar "Export 3MF" has the same, and, while a feature command is open (`aria-disabled="true"`), `border` and `COLOR_CLASSES.forcedEdge.border` without the `accent` border or background. The read-only JSON block of an unsupported field has `rounded-md border`, `COLOR_CLASSES.tonal.background` and `COLOR_CLASSES.forcedEdge.border`. A `TextField`, a choice shown as a select, a `selection-field`, a `Switch`, each swatch of `color-popover`, the overhang legend's ramp bar and each radio of the export popover's Bodies group keep `COLOR_CLASSES.borderControl.border` (or, in their states, the `accent`, `errorMark` or `warningMark` border; a checked Bodies radio is `border-4` with `COLOR_CLASSES.accent.border`); a `Switch`'s thumb has `border` and `COLOR_CLASSES.forcedEdge.border`; each swatch of `color-popover` and the ramp bar have `forced-color-adjust-none`, and no other element has a `forced-color-adjust-` or `forced-colors:` class; the palette's input has `COLOR_CLASSES.textMuted.placeholder` and no `border` class (3.24).
212. U-212. Overflow edges: `useOverflowEdges` on an element whose `clientHeight` is 100 and `scrollHeight` 300 (stubbed): at `scrollTop` 0 it has `data-overflow-bottom` and not `data-overflow-top`; after a `scroll` event at `scrollTop` 50 it has both; at 200, and at 199.5, only `data-overflow-top`; with `scrollHeight` 100 and the `ResizeObserver` callback run, neither; appending a child and running the `MutationObserver` callback with `scrollHeight` 300 again sets `data-overflow-bottom`; after unmount, `scroll` events change nothing. Rendering `App` with fakes, the scrolling bodies of 3.2 carry their rule: the bodies of the Parameters, Problems and Keyboard and mouse panels (the element after `PanelHeader`'s), the print check panel's list and the selection inspector's body are `border-t` with `COLOR_CLASSES.forcedEdge.border` and `COLOR_CLASSES.divider.overflowTop`; the command inspector's body is `border-y` with both `divider` overflow facets; the palette's list is `border-b` with `COLOR_CLASSES.divider.overflowBottom`; and no other element has an overflow facet.
213. U-213. State outlines, rendering `App` with fakes and `cube.ptsm` open: with the body row "Cube" selected, the row has `COLOR_CLASSES.selection.background`, `outline`, `-outline-offset-1` and `COLOR_CLASSES.forcedEdge.outline`, and the other rows `COLOR_CLASSES.forcedEdge.outline` without `outline`; clicking the timeline item "Extrude 1" gives it the same three classes with `COLOR_CLASSES.selection.background`, and double-clicking it (editing) gives it `outline-2`, `-outline-offset-2` and `COLOR_CLASSES.accent.outline` and neither `-outline-offset-1` nor `COLOR_CLASSES.forcedEdge.outline`; pressing "Orthographic" in `viewport-controls` (unpressed and without `outline` before) gives it `aria-pressed="true"`, `COLOR_CLASSES.selection.background` and the three classes; with the theme setting `dark`, the top bar's "Toggle browser" (the browser open) and `theme-toggle` have `aria-pressed="true"`, `COLOR_CLASSES.hover.active`, `COLOR_CLASSES.text.foreground` and the three classes and not `COLOR_CLASSES.selection.background`, as an `IconButton` with `pressedFill="neutral"` has when pressed; in the palette, after typing "ext", the active option has `COLOR_CLASSES.hover.active` and the three classes and no other option has `outline`; in the Document menu after ArrowDown, the keyboard-active item has `COLOR_CLASSES.hover.active` and the three classes. No element has both `COLOR_CLASSES.accent.outline` and `COLOR_CLASSES.forcedEdge.outline`.
214. U-214. Labels under the free area's cards (3.12): `hideLabelsUnderCards(labels, freeArea)` with one `data-card` element in `freeArea` at (100, 100, 200, 80) (`getBoundingClientRect` stubbed) hides a shown 16 by 16 label at (290, 170), keeps one at (300, 100), whose box only touches the card's edge, and one at (20, 20); hides a wrapper at (20, 20) whose first child is at (120, 120); and changes nothing with `freeArea` null or without a `data-card` element. Rendering `App` with fakes, in the first setup of U-209 the elements with `data-card` are exactly `view-cube`, its "Home view" button, `viewport-controls` and `inspector`; in an empty document `empty-state` has it, and in sketch mode `sketch-header` has it, and `sketch-hint` has it exactly when it shows a hint (not `sr-only`).
215. U-215. Sketch mode's own chunk (3.1), with the shell rendered by `createRoot` outside `act` (whose queue drops a render that waits for data) over fakes in sketch mode on `rect-sketch.ptsm`, and the module `SketchOverlay.tsx` held back until the test lets it load: `app` has `data-context="sketch"` with no `sketch-header` and `loadedSketchOverlay()` null; once the module loads, `sketch-header` and `sketch-overlay` appear, `loadedSketchOverlay()` is its `SketchOverlay`, and `loadSketchOverlay()` resolves with it.

Assets and files changed elsewhere (3.19, 4.4, ADR 0037). These tests use the fakes of `src/test/fakes`. The fake core keeps the assets `open_document` supplies for each document, answers `save_document` with those whose name occurs in the saved text (sorted, as `u8` buffers), gives an `import` record whose `asset` it does not hold the status error with an `asset_missing` message, and refuses with `asset_mismatch` an `open_document` that supplies a name in its `mismatchedAssets` option. The fake file service keeps each handle's bytes, which a test changes to stand for a change made elsewhere; keeps the asset files of fake folders (`createFakeFolderHandle`) and of its desktop handles; records every call with its arguments; and answers `chooseFolder` from a queue. `A` is the asset name of 64 `a` digits followed by `.step`, holding the bytes 1, 2, 3, and `B` that of 64 `b` digits, holding 4, 5. `block.ptsm` holds a document with body `b-1` "Body 1" and `f-imp1`, an `import` of `A` into it, op new.

216. U-216. `fileDigest` of the UTF-8 bytes of `hello` is `2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824`, of no bytes `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`, and of a view of bytes 1 to 5 of a 16-byte buffer the digest of those 5 bytes alone.
217. U-217. `referencedAssetNames` of `{"features": [{"asset": "<B>"}, {"asset": "<A>", "name": "<A>"}], "<A>": 1, "list": ["<A with STEP in capitals>", "x.step", "<63 a digits>.step", 7]}` is `[A, B]`: each name once, sorted, and not the key, the capitalized extension, the short name or the number. Of `not json` it is `[]`. `ASSET_NAME` matches `A` and does not match `A` followed by a line feed.
218. U-218. Asset plumbing in the core API, with a fake transport: `openDocument(text, [A, B])` sends a request frame whose command is `{type: 'open_document', text, assets: [{name: A, data: 0}, {name: B, data: 1}]}`, with two `u8` buffers holding 1, 2, 3 and 4, 5; `openDocument(text)` sends `assets: []` and no buffer. A `save_document` reply with `assets: [{name: A, data: 0}]` and one `u8` buffer resolves `saveDocument` with `assets` `[{name: A, bytes: 1, 2, 3}]`, `text` and `revision`; writing into the reply frame's buffer afterwards leaves those bytes unchanged.
219. U-219. Open reads the assets the text names: with `block.ptsm` opened from a desktop handle whose folder holds `A` and `B`, `readAssets` is called once, with the place `{handle, folder: null}` and the names `[A]`, before `openDocument`, which receives `[A]` only. `DocumentState.assets` is `[A]`, "Import 1" is ok and no notification is shown. Opening `cube.ptsm` from the same handle calls no `readAssets` and sends `assets: []`.
220. U-220. A folder that cannot be read: (a) with `readAssets` answering no assets and the failure `FileServiceError('io', 'Disk failure')` with `entry` `assets/<A>`, `block.ptsm` opens with `assets: []` and shows the warning `assets_not_read` ("Partsmith could not read some imported files from the assets folder next to block.ptsm, so their geometry is missing.") with "Copy details", which copies the lines `assets_not_read`, that message and `assets/<A>: Disk failure`; no other assets notification follows, since the folder was reached. (b) With the fake core's `mismatchedAssets` `[A]`, `openDocument` is sent twice, with `[A]` and then with no assets; the document opens with "Import 1" in error, `open_failed` is not shown, and the warning `assets_mismatch` offers "Copy details", whose detail is the core's message. `DocumentState.assets` is `[]`, and a following `app.restartCore()` sends `open_document` with `assets: []` and no buffer. (c) From a web handle with a folder `F`, a `readAssets` failure `FileServiceError('permission', 'Partsmith has no access to the folder.')` with `entry` `assets` shows `assets_folder_needed` with "Choose folder" and no `assets_not_read`.
221. U-221. Why imports are missing: `block.ptsm` opened (a) from a web handle without a folder shows the persistent warning `assets_folder_needed` ("The imported geometry of block.ptsm is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder block.ptsm is in.") with "Choose folder" and `features` `['f-imp1']`; (b) dropped (no handle) with `fileSystemAccess` true, `assets_without_folder` with "Open..."; (c) dropped with `fileSystemAccess` false, `assets_unsupported` with no action; (d) from a desktop handle whose folder lacks `A`, none of these and no `assets_not_read`. `cube.ptsm` opened from a web handle without a folder shows none. After (a), a recompute in which `f-imp1` is no longer in error (its suppression) dismisses the warning. (e) A recovery entry of `block` without a handle, opened with its startup "Open", shows `assets_without_folder` reading "The imported geometry of block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead."
222. U-222. Choose folder: after U-221 (a), the body renamed to "Block" (so the document is unsaved), "Choose folder" with `chooseFolder` answering `other` named `parts` shows the error `assets_wrong_folder` ("parts is not the folder that holds block.ptsm. Choose the folder block.ptsm is in.") with "Choose folder". That action, answering `chosen` with a folder `F` that holds `A`, calls `readAssets({handle, folder: F}, [A])`, then sends `open_document` with `JSON.stringify(settled recipe, null, 2)` (the body named "Block") and `[A]`, then `close_document` of the old handle, in that order. The shown document has the new handle, the same name and `recoveryId`, a file reference with folder `F`, `assets` `[A]` and `localUnsaved` true; recent files hold the handle with `F`; and neither `assets_folder_needed` nor `assets_wrong_folder` is shown. Without the rename, the reopened document has `localUnsaved` false and `data-dirty="false"`. `canceled` sends nothing and shows `assets_folder_needed` again with `features` `['f-imp1']`. A chosen folder `G` without `A` calls `readAssets({handle, folder: G}, [A])`, sends no `open_document` and shows no notification. The action has `editsDocument` true; run during a Fillet command, it calls no `chooseFolder` and shows `finish_command_first`. A "Choose folder" clicked after New replaced the document calls no `chooseFolder`. While `readAssets` of `F` has not answered: a rename to "Block" committed then is in the `open_document` text and the reopened recipe; a Fillet command started then leaves the document and the command as they are, sends no `open_document`, and shows `finish_command_first` and `assets_folder_needed` again with `features` `['f-imp1']`; and a New sends no `open_document`. A rename still waiting for the core (paused) when the folder is chosen is sent before `open_document`, whose text has "Block", and a rename made while `open_document` has not answered is sent once it has, with the reopened document's handle, so the reopened document is named "Block" and unsaved.
223. U-223. The change check: `cube.ptsm` opened from a web handle `H` with bytes `T` has the file reference digest `fileDigest(T)`; then the body is renamed to "Box". (a) With `H` unchanged, Save calls `saveDocument`, `readHandle(H, 'readwrite')`, `write` and `markSaved`, in that order, and the digest becomes that of the written bytes. (b) With `H` rewritten with the same bytes `T`, the same. (c) With `H` holding other bytes `T2`, Save calls `saveDocument` and `readHandle` and no `writeAssets`, `write` or `markSaved`; `data-dirty` stays true, and the error `save_conflict` reads "cube.ptsm was changed elsewhere after you opened or saved it, so Partsmith did not save over it." with the actions "Overwrite cube.ptsm", "Save as" and "Open the newer file", in that order. (d) A `readHandle` that throws `not_found` shows `save_not_found` and one that throws `permission` shows `save_permission`, each without a `write`. (e) With `crypto.subtle.digest` rejecting, opening `cube.ptsm` from `H` gives the file reference digest null and logs one `console.warn`, and Save calls `saveDocument`, `write` and `markSaved` with no `readHandle`. (f) With the first `write` held: a second Save, pressed after a rename to "Crate", sends no `save_document` until the first save has sent `markSaved`, then reads `H`, writes the text holding "Crate" and marks it saved, with no `save_conflict` and the digest of the second write; a Save pressed while a Save as writes to `H2` writes to `H2` after it, reading `H2` and leaving `H` unchanged; and a Save waiting behind another write when New replaces the document sends nothing.
224. U-224. The conflict's actions, from U-223 (c): "Overwrite cube.ptsm" saves again, calling `readHandle`, then `write` with the text holding "Box", then `markSaved`; `data-dirty` is false, and a Save after another rename with `H` unchanged shows no `save_conflict`. When `H` changes again to `T3` before the click, "Overwrite cube.ptsm" shows `save_conflict` again and writes nothing. "Open the newer file" calls `readHandle(H)` and shows the document of `T2` with the file reference digest `fileDigest(T2)`; the recovery store holds the old session's entry as `replaced`, and `stashed` ("cube had unsaved changes. They are kept for you.") offers "Reopen". "Save as" calls `chooseSaveTarget('cube.ptsm', 'ptsm')` and writes to its target `H2` with no `readHandle`. "Overwrite cube.ptsm" has `editsDocument` true. Each of the three actions, clicked after New, or after a Save as to another file `H2` succeeded, calls no `readHandle`, `chooseSaveTarget`, `writeAssets` or `write` and sends no request.
225. U-225. No check without a known digest: a recovery entry without `digest` (a record written before it existed) opened with "Open" saves to its handle without `readHandle`; one with `digest` `D` opens with the file reference digest `D`, so Save shows `save_conflict` when the handle's bytes no longer hash to `D`, and writes when they do.
226. U-226. Save writes the assets first: `block.ptsm` opened from a desktop handle `H` whose folder holds `A`, with the body renamed: Save calls `saveDocument`, `readHandle`, `writeAssets({handle: H, folder: null}, [A])`, `write` and `markSaved`, in that order; for `cube.ptsm`, whose `save_document` reply has no assets, it calls no `writeAssets`. With `writeAssets` throwing `FileServiceError('permission', 'Partsmith has no access to <A>: denied')` with `entry` `assets/<A>`, Save shows the error `save_assets` ("block.ptsm was not saved, because Partsmith could not write its imported files to the assets folder next to it.") with "Save as" and "Copy details", whose detail is `assets/<A>: Partsmith has no access to <A>: denied`, calls no `write` or `markSaved`, and `data-dirty` stays true.
227. U-227. Save as into another web folder: `block.ptsm` opened from a web handle `H` and given folder `F` with "Choose folder" (assets `[A]`); Save as to a handle `H2` that `F` does not hold sends `save_document`, writes nothing and shows the persistent warning `save_folder_needed` with "Choose folder". Its action, with `chooseFolder(H2)` answering `chosen` with `G`, sends `save_document` again and calls `writeAssets({handle: H2, folder: G}, [A])`, `write` to `H2` and `markSaved`, in that order; the document's file is then `H2` with folder `G` and the digest of the written bytes, and recent files hold `H2` with `G`. With `other`, it shows `assets_wrong_folder`, whose "Choose folder" asks for `H2`'s folder again; with `canceled`, nothing is written, the document's file stays `H`, and `save_folder_needed` is shown again, whose "Choose folder" asks for `H2`'s folder. That "Choose folder" clicked after New, or after a Save as to another file `H4` succeeded, calls no `chooseFolder`, `writeAssets` or `write`. Save as to a handle `H3` that `F` holds writes at once, with `writeAssets({handle: H3, folder: F}, [A])`, and shows no warning.
228. U-228. A restart keeps the assets: with `block.ptsm` opened from `H` and given folder `F` (U-227), so its assets are `[A]` and its digest is `D`, `app.restartCore()` sends the new core `open_document` with `JSON.stringify(recipe, null, 2)` and `[A]` (a buffer holding 1, 2, 3) and calls no `readAssets`. The document keeps its name, file reference (`H`, `F`, `D`), assets and `recoveryId`, has `localUnsaved` true, and "Import 1" is ok on the new core.
229. U-229. Recovery keeps the place: the recovery entry that an unsaved `block.ptsm` from `H` with folder `F` and digest `D` writes holds `H`, `F` and `D`. Its startup "Open" calls `readAssets({handle: H, folder: F}, [A])`, sends `open_document` with `[A]` and gives the document the file reference (`H`, `F`, `D`); "Reopen" of its `stashed` notification does the same. The IndexedDB recovery store (fake IndexedDB) round-trips an entry with a folder handle and a digest, lists a version 2 record without `folder` and `digest` as an entry with both null, and skips a record whose `folder` is `{kind: 'file'}`. The recent store keeps `folder` and reads a record without it as null; remembering `H` with `folder` null after an entry of `H` with `F` keeps `F`. With that entry, `openFile` of `block.ptsm` read from `H` without a folder (as the picker gives it) calls `readAssets({handle: H, folder: F}, [A])`.
230. U-230. Web `readAssets`, with fake handles: a place without a folder resolves `{assets: [], failure: null}` and calls nothing. With a folder `F` whose `assets` holds the file `A` (1, 2, 3) and a folder named `B`, the names `[A, B, C]` give the assets `[A]` and an `io` failure with `entry` `assets/<B>`, and `C`, which is not there, is left out without a failure. `F` without `assets` gives no assets and no failure, and `F` with a file named `assets` gives an `io` failure with `entry` `assets`. When `F.queryPermission({mode: 'readwrite'})` is `prompt` it calls `requestPermission({mode: 'readwrite'})`, and `denied` gives a `permission` failure with `entry` `assets`.
231. U-231. Web `writeAssets` and its entry check, with fake handles and a folder `F` without `assets`: `[A]` creates `assets` and writes the file `A` with 1, 2, 3 through one `createWritable`. Run again with `A` unchanged, it calls no `createWritable`. `A` holding 1, 2, 4 (the same size) is replaced, and `A` holding 9 (another size) is replaced without the `arrayBuffer` of its fake `File` being called. With `A` a folder, it throws an `io` `FileServiceError` with `entry` `assets/<A>`, the folder stays, and `B` of `[A, B]` is not written. A new entry whose writable's `write` rejects is removed (`removeEntry(A)`), and the error has `entry` `assets/<A>`; an existing file whose `write` rejects is aborted (`abort()`) and keeps its bytes. A place without a folder throws `io` "Partsmith has no access to the folder of block.ptsm." with `entry` `assets` and calls nothing.
232. U-232. Web folders and permissions: `chooseFolder(H)` calls `showDirectoryPicker({ mode: 'readwrite', startIn: H })`; an `AbortError` gives `canceled`; a folder whose `resolve(H)` is `['block.ptsm']` gives `chosen`; one whose `resolve(H)` is `null`, `['sub', 'block.ptsm']`, or throws gives `other` with the folder's name. `folderHolds` of a desktop handle is false. `readHandle(H, 'readwrite')` queries and requests `readwrite`, and `readHandle(H)` `read`. `readHandle(H, 'read', F)` with both answering `prompt` calls `F.requestPermission({mode: 'readwrite'})` before `H.queryPermission`, and reads the file when `F` answers `denied` and `H` grants it. `reachesFolder` is true only for a web handle with a folder.

Print suggestions (3.17, print.md 3.12, ADR 0040):

233. U-233. `suggestionText` gives each shared case of print.md 3.12 its words (`{kind: 'split', planes: [{origin, normal}, ...]}` with the case's planes, or `{kind: 'drain_hole', at}` with its point), one test per case, and null for the split without planes (`{kind: 'split', planes: []}`). It takes no unit: the words are the same in a document in inches.
234. U-234. `splitPlaneOutline(plane, box)`: in the box (0, 0, 0) to (300, 100, 10), the plane through (150, 50, 5) with normal (1, 0, 0) gives exactly the corners (150, 0, 0), (150, 100, 0), (150, 100, 10) and (150, 0, 10), in an order where every two neighbors (the last and the first included) lie on one face of the box. In the box (0, 0, 0) to (10, 10, 10), the plane through (5, 5, 5) with normal (1, 1, 1)/√3 gives the six corners (10, 5, 0), (5, 10, 0), (0, 10, 5), (0, 5, 10), (5, 0, 10) and (10, 0, 5), each once, in that cyclic order or its reverse; the plane through (10, 5, 5) with normal (1, 0, 0) gives the four corners of the face x = 10, each once; the plane through (10, 10, 5) with normal (1, 1, 0)/√2, which touches the box only along an edge, and the plane through (20, 5, 5) with normal (1, 0, 0), which misses it, give null.
235. U-235. Rows with suggestions, rendering `HealthList` from `healthRows`: a report whose body "Slab" has a `bed_fit` finding with message M, a bounds location and the suggestion `{kind: 'split', planes: [{origin: [0, 0, 5], normal: [1, 0, 0]}, {origin: [0, 0, 5], normal: [0, 1, 0]}]}`, whose body "Mushroom" has an `overhangs` finding with message N and no suggestion, and whose body "Peg" has a `bed_fit` finding with message P and the suggestion `{kind: 'split', planes: []}`. `healthRows` gives the Slab row `suggestion` "Split at x = 0 mm and y = 0 mm" and the Mushroom and Peg rows `suggestion` null. The Slab row's `health-suggestion` reads "Split at x = 0 mm and y = 0 mm" and comes after M and before "Slab" in the row's text, and its accessible name is "Error: M Split at x = 0 mm and y = 0 mm. Slab"; the Mushroom row has no `health-suggestion` and the name "Warning: N Mushroom", and the Peg row has no `health-suggestion` and the name "Error: P Peg". With the document's units set to `in` on the same report, the Slab row's `health-suggestion` still reads "Split at x = 0 mm and y = 0 mm".
236. U-236. Focusing a suggestion: body "Slab" with the mesh box (0, 0, 0) to (400, 400, 10) and a `bed_fit` finding whose location is that box and whose suggestion is a split along the planes through (200, 200, 5) with normals (1, 0, 0) and (0, 1, 0). `focusFinding` calls `zoomToBounds` once, with that box and margin 0.3 (where the cube rule would frame a cube of side 0.25 × the box diagonal, about 141 mm, around (200, 200, 5)); `focusedFindingPoint` is (200, 200, 5); `focusedSplitOutlines` holds two outlines of four corners each, the first in the plane x = 200 and the second in the plane y = 200. The same finding with the suggestion `{kind: 'split', planes: []}` frames that cube instead, and `focusedSplitOutlines` is empty. Body "Hollow" with the mesh box (-10, -10, 0) to (10, 10, 20) and a `trapped_volumes` finding whose location is the box (-8, -8, 2) to (8, 8, 18) and whose suggestion is a drain hole at (-8, -8, 2): `focusFinding` frames the cube of side 20 mm centered at (-8, -8, 2) with margin 0.3, `focusedFindingPoint` is (-8, -8, 2), and `focusedSplitOutlines` is empty. With `installPrintAnalysis` running and the Slab finding focused, each of these empties `focusedSplitOutlines` and makes `focusedFindingPoint` null: closing the print check panel, a new document revision, a change of `print.orientation` (to a face down, and back to as modeled), hiding the `bed_fit` check, and a new report replacing the one the finding was taken from (after a new document revision 4, the Slab finding is focused again from the stale report's row, which shows its two planes; then a report of revision 4 arrives whose `bed_fit` finding at the same key is split along the plane through (200, 200, 5) with normal (0, 0, 1)). Rendering `Viewport` with fakes, `viewport` has `data-split-planes="0"`, then "2" after the Slab finding is focused, then "0" after the panel closes.
237. U-237. Split planes in the scene (3.12 pass 7), with U-236's Slab finding focused: the frame holds, for each of the two planes, a fill draw and an outline draw on the `overlay` layer with `depthTest` false, the `problem` color of the theme, opacity 0.15 for the fill and 0.6 for the outline, the outline 1.5 × the UI scale wide, and no pick base. The fill's geometry is `fill` geometry whose positions are the outline's corners in order and whose indices are the fan from the first corner (`0, 1, 2, 0, 2, 3` for four corners); the outline's is `lines` geometry with one segment from each corner to the next, the last back to the first (6 floats per segment). With no focused finding, with the Hollow finding of U-236 focused, and after the panel closes, the frame holds none of them and their geometry is no longer uploaded.

### 6.5 Playwright end-to-end tests

Each test starts with `openApp` unless it says otherwise. "Click X" means clicking the element with accessible name X in the region named. Positions are relative to the center `(cx, cy)` of the free area (the `viewport` box, `canvas(page)`, 6.3) in CSS pixels, with `H` its height. A test that states the text of `body-size`, `body-center` or `vertex-position` ("20 × 20 × 20 mm", "0, 0, 10 mm") compares their `data-value`; their visible text carries the axis letters (3.8, U-182).

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
17. E-17. Drop `rect-sketch.ptsm`, double-click "Sketch 1": `data-dof="0"`. With `s = sketchToScreen` for content (0, 0) to (40, 20): run "Equal" from the palette, click the bottom line at `(cx, cy + 10 s)` and the left line at `(cx - 20 s, cy)` (both lie on the sketch's axes; the lines are picked, 3.6): a `notification[data-severity=warning]` reads "Equal conflicts with Distance 40 and Distance 20, so it was not added."; `data-dof="0"` and two `sketch-dimension`s. Click "Replace them": one `constraint-glyph[data-type=equal]`, no `sketch-dimension`, `data-dof="1"`. (Horizontal and Vertical on one line do not conflict: their rows are independent, so the solver collapses the line, sketch.md 3.8.)
18. E-18. Drop `rect-sketch.ptsm`, double-click the timeline item "Sketch 1": `data-state="full"`. With `s = sketchToScreen` for content (0, 0) to (40, 20): press D, click the top line at `(cx, cy - 10 s)`, click `(cx, cy - 10 s + 30)` (inside the rectangle, clear of the top-center slot's chips above it), type "40", Enter: the editor shows the `dimension_over_constrained` text in a `role=alert`. Click "Add reference": a new `sketch-dimension` with `data-driving="false"` and text "(40)".
19. E-19. New sketch on XY. Holding Control, draw line A from `(cx - 150, cy + 60)` to `(cx + 150, cy + 60)` and line B from `(cx - 100, cy - 60)` to `(cx + 100, cy - 140)`, leaving the tool after each: `data-dof="8"`. Click A at `(cx, cy + 60)`, Shift+click B at `(cx, cy - 100)`: the selection inspector has buttons Parallel, Perpendicular, Collinear, Equal. Click "Perpendicular": `data-dof="7"` and one `constraint-glyph[data-type=perpendicular]`.
20. E-20. Drop `underconstrained.ptsm`, double-click "Sketch 1": `data-dof="7"`. With the mapping for content (0, -5) to (55, 5), the line end (30, 0) is a `sketch_point` on hover. Drag it in 10 steps to (30, 10) mm and release: `data-dof="7"`; hovering the new position gives `sketch_point`, the old one does not. ControlOrMeta+Z: hovering the old position gives `sketch_point` again.
21. E-21. Drop `rect-sketch.ptsm`, enter the sketch. Drag on empty space from 30 pixels above and left of the rectangle's top-left corner to 30 pixels below and right of its bottom-right corner: `selection-title` "4 lines". Drag from right to left across only the bottom line's middle: `selection-title` "1 line".
22. E-22. In sketch mode press ControlOrMeta+Enter: `data-context="model"`; the browser shows a button "Hide Sketch 1" (the sketch stays visible).

Inspector, timeline and browser:

23. E-23. Drop `parametric.ptsm`; click the body row "Cube": `body-size` "20 × 20 × 20 mm". Run "Parameters"; fill "Expression of size" with "30 mm", Enter: `parameter-value` "30 mm" and `body-size` "30 × 30 × 30 mm". Click the panel's title "Parameters" (focus leaves the expression field, where `Mod+Z` is the field's own text undo, 2.10 dispatch rule 2), ControlOrMeta+Z: "20 × 20 × 20 mm".
24. E-24. Drop `cube-features.ptsm`; drag `timeline-marker` to the gap between the first and second items: items 2 to 4 have `data-rolled-back="true"`, the marker's `aria-valuenow` is 1, the body row reads "Cube (empty)", and hovering `(cx, cy)` does not give `face`. ControlOrMeta+Z: `aria-valuenow` 4 and no rolled-back items.
25. E-25. Drop `cube-features.ptsm`; focus the marker; ArrowLeft: `aria-valuenow` 3; End: 4 with `aria-valuetext` "At the end"; Home: 0 with `aria-valuetext` "At the start", and every `timeline-item` has `data-rolled-back="true"`.
26. E-26. Drop `two-bodies.ptsm`; drag "Cylinder 1" to the first gap: item names in order "Cylinder 1", "Sketch 1", "Extrude 1", all ok. Drag "Extrude 1" to the gap between "Cylinder 1" and "Sketch 1": "Extrude 1" has `data-status="error"` and an error notification starts with "Extrude 1 failed."; click its "Undo": the order is "Cylinder 1", "Sketch 1", "Extrude 1" and "Extrude 1" is ok.
27. E-27. Drop `cube-features.ptsm`; right-click "Fillet 1", choose "Suppress": `data-suppressed="true"`, `data-status="suppressed"`. ControlOrMeta+Z: `data-suppressed="false"`.
28. E-28. Drop `fillet-error.ptsm`: "Fillet 1" has `data-status="error"`; `timeline-problems` reads "1 problem"; clicking it opens `problems-panel` listing "Fillet 1" with a message containing "Fillet radius"; clicking "Edit" opens the inspector "Fillet 1" whose `inspector-error` has `role=alert`.
29. E-29. Drop `cube.ptsm`; double-click "Extrude 1": the inspector "Extrude 1" with the Distance input value "20 mm". Fill "30" and wait for `data-preview="shown"`; Escape: the command closes (`inspector` no longer has `data-mode="command"`; the double-click's clicks selected "Extrude 1", so the selection inspector shows it, 3.8, 3.9) and `body-size` (select the body) is "20 × 20 × 20 mm". Double-click "Extrude 1", fill "30", Enter: "20 × 20 × 30 mm". ControlOrMeta+Z: "20 × 20 × 20 mm".
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

41. E-41. Drop `cube.ptsm`. Click "Top view": `data-view="top"`; "Home view": `iso`; "Front view": `front`; "Front right top corner view": `iso` (each face is clicked while it faces the viewer: seen from the top, the Front face is turned away). Shift+middle-drag 100 pixels on the canvas: `custom`.
42. E-42. Click "Orthographic": `aria-pressed="true"` and `data-projection="orthographic"`. Press Shift+P: `perspective`.
43. E-43. Drop `cube.ptsm`, front fit. Middle-drag from `(cx, cy)` to `(cx + 0.6 H, cy)`: `data-camera-target` changed, hover `(cx, cy)` gives `none` and `(cx + 0.6 H, cy)` gives `face`. Press F6, then five wheel events of `deltaY: -100` at `(cx, cy)`: hover `(cx, cy + 0.45 H)` gives `face`.
44. E-44. Run "Navigation: FreeCAD": `data-nav-preset="freecad"`. Front fit; press the middle button, then the left button, move 100 pixels, release both: `data-view="custom"`. Click "Front view"; middle-drag 100 pixels: `data-view` stays `front` and `data-camera-target` changes.
45. E-45. Drop `cube.ptsm`, front fit, press Shift+X: `section-popover` visible; fill Offset with "10", Enter: `data-section="on"`; hover `(cx, cy - 0.2 H)` gives `none` and `(cx, cy + 0.2 H)` gives `face`. Escape closes the popover (while it is open only `Mod+K` is a shortcut, 2.10). Shift+X: `data-section="off"` and `(cx, cy - 0.2 H)` gives `face`.
46. E-46. Drop `cube.ptsm`, front fit; ten wheel events of `deltaY: 100`: hover `(cx, cy - 0.39 H)` gives `none`. Press F6: `(cx, cy - 0.38 H)` gives `face` and `(cx, cy - 0.42 H)` gives `none`.

Print and export:

47. E-47. Drop `overhang.ptsm`; click the radio "Print": `data-context="print"`; the toolbar "Print tools" has buttons starting "Printer:", "Overhang map" (`aria-pressed="true"`), "Checks", "Print check", "Lay on face", "Export 3MF", "Export". `health-badge` reaches `data-state` `warning` or `error` with `data-stale="false"`; `overhang-legend` is visible and contains "°". Click the badge: `health-list` has at least one `health-item[data-check=overhangs]`. Clicking it changes `data-camera-target` and shows `finding-marker`.
48. E-48. Drop `cube.ptsm`, go to Print, click the "Printer:" button: `printer-menu` has at least two `menuitemradio`s. Choose one that is not checked: `status-printer` shows its name. ControlOrMeta+Z: the previous name.
49. E-49. Drop `oversize.ptsm`, go to Print: `health-badge` `data-state="error"`; click "Print check": a `health-item[data-check=bed_fit][data-severity=error]`.
50. E-50. Drop `overhang.ptsm`, go to Print, click "Print check": at least one `health-item[data-check=overhangs]`. Open "Checks", switch "Overhangs" off: no `health-item[data-check=overhangs]`, and `overhang-legend` is still visible. Close the popover and click "Overhang map": no `overhang-legend`. Reload, drop the file, go to Print: the switch is still off.
51. E-51. Drop `overhang.ptsm`, go to Print and wait for a report; click "Model", rename the body: `health-badge` `data-stale="true"`. Click it: `data-stale="false"` and `health-list` visible.
52. E-52. Without File System Access, drop `cube.ptsm`, go to Print, click "Export 3MF": a download `cube.3mf` starting with bytes `50 4b 03 04`, and a notification "Exported cube.3mf."
53. E-53. Open the export popover, choose STL, click "Export": a download `cube.stl` whose length is 84 plus 50 times the little-endian `u32` at offset 80, which is at least 12.
54. E-54. STEP: `cube.step` starting with `ISO-10303-21;`. OBJ: `cube.obj` containing a line starting `v ` and a line starting `f `.
55. E-55. Drop `cube.ptsm`, click "Hide Cube", go to Print, click "Export 3MF": the notification reads "There is no visible body with geometry to export." and no download starts within 2 s.

Themes:

56. E-56. Run "Theme: Dark": `html` has the class `dark`; reload: still dark. Run "Theme: Light": `html` has no `dark` class. With `page.emulateMedia({ colorScheme: 'dark' })`, run "Theme: System": `html` has the class `dark`.

Feature coverage (E-57, one test per row, parametrized). Fields are found by the labels in each feature's manifest table (`docs/spec/features/<type>.md`); selection fields are filled by the clicks listed, and a field that already holds the listed item (a starting value, 2.12) is left as is. Each test ends with Enter where the Enter ladder confirms a command (3.5: in the inspector, not on a button, or on the viewport; a row whose last step clicks a browser row gives the viewport focus first, since Enter on a focused browser row activates that row, 3.10) and expects a new timeline item with the feature's default label and `data-status="ok"`, plus the listed result.

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

58. E-58. Drop `fillet-error.ptsm`; open the problems panel and click "Edit" on "Fillet 1": `inspector-error` has a button whose name starts with "Use "; click it (it leaves focus on the radius input, 3.7), wait for `data-preview="shown"`, press Enter: the command closes (`inspector` no longer has `data-mode="command"`; the preview already showed "Fillet 1" ok) and "Fillet 1" has `data-status="ok"`.
59. E-59. Drop `parametric.ptsm`; run "Parameters"; set "Name of size" to "side", Enter: every `timeline-item` has `data-status="ok"`; click the panel's title "Parameters" (as in E-23), then ControlOrMeta+Z restores the name "size".

Theme, styles and fonts:

60. E-60. Theme toggle: with `page.emulateMedia({ colorScheme: 'light' })`, `openApp`: `html` has no `dark` class and `theme-toggle` has `aria-pressed="false"`. Click it: `html` has the class `dark`, `aria-pressed="true"`, `localStorage.getItem('partsmith.settings.v1')` parses to an object with `theme` `dark`, and the computed `background-color` of `top-bar` differs from its value before the click. Reload: `html` has the class `dark` as soon as `app` is ready. Click it again: no `dark` class and the stored `theme` is `light`. Emulate `colorScheme: 'dark'` and reload: still no `dark` class (the explicit choice wins). Run "Theme: System": `html` has the class `dark`.
61. E-61. No inline styles except runtime positions: in each of these states, every element matching `[style]` has `data-positioned`, and its `style` attribute declares only `translate`, except the view cube rotor, which declares only `transform`: (a) right after `openApp`; (b) `rect-sketch.ptsm` dropped and "Sketch 1" double-clicked, with at least one `sketch-dimension` and one `constraint-glyph` visible, each the child of an element with `data-positioned` (the wrapper of 3.12, since the inline `translate` would replace their own centering translate classes); (c) `cube.ptsm` dropped, "Extrude 1" double-clicked (the `inspector` has `data-positioned`) and the Document menu open; (d) `overhang.ptsm` dropped, Print context with `overhang-legend` visible.
62. E-62. Fonts (2.16.7). "First family" is the computed `font-family` up to its first comma, without quotes. Drop `rect-sketch.ptsm` and double-click "Sketch 1": the first family of `document-name`, `status-units`, `sketch-header` and the first `sketch-dimension` is `Geist`; the computed `font-variant-numeric` of the first `sketch-dimension` and of `status-dof` is `tabular-nums`, and that of `sketch-header` is `normal`. Finish the sketch and press E: the first family of `expression-result`, of the `inspector-field` label "Distance" and of the input "Distance" is `Geist`, and the `font-variant-numeric` of the input and of `expression-result` is `tabular-nums`. Press Escape, drop `parametric.ptsm` and run "Parameters": the first family of the inputs "Name of size" and "Expression of size" is `JetBrains Mono`, their computed `font-variant-ligatures` is `no-contextual` and their `font-feature-settings` is `normal`; the first family of `parameter-value` is `Geist` with `font-variant-numeric` `tabular-nums`; the element holding the browser parameter row's text "size = 20 mm" has `JetBrains Mono` first and `font-variant-ligatures` `normal`. Open the palette and type "size": the title of the option with `data-kind="parameter"` has `JetBrains Mono` first, and its kind tag "Parameter" has `Geist` first. Then `document.fonts.check('14px Geist')` and `document.fonts.check('14px "JetBrains Mono"')` are true, the `FontFace` of `document.fonts` whose family (quotes removed) is `Geist` has `weight` `100 900`, `display` `swap` and `status` `loaded`, and the one of `JetBrains Mono` has `weight` `100 800`, `sizeAdjust` `96%` and `status` `loaded`.

One interface for everyone (open issue 15):

63. E-63. Cold start: `empty-state` is visible, contains "Start with a sketch on a plane, or add a box." and "right-drag or drag the view cube", and holds buttons "Sketch" and "Box"; `timeline-empty` reads "Each step you add appears here. Double-click a step to change it later."; expanding "Origin" in the browser shows the button "Hide X axis". Click "Box" in `empty-state`: `empty-state` is hidden and the inspector "Box" is open. Press Enter: the command closes (`inspector` no longer has `data-mode="command"`; the preview already put "Box 1" in the timeline and hid both empty states), a `timeline-item` "Box 1", no `empty-state`, no `timeline-empty`, and the button "Show X axis". ControlOrMeta+Z: `empty-state` is visible.
64. E-64. Cold start, click "Sketch" in `empty-state`: `data-context="sketch"`, `sketch-header` `data-state="picking"`, no `empty-state`. Escape: `data-context="model"` and `empty-state` is visible.
65. E-65. Drop `cube.ptsm`, front fit. Right-drag 100 pixels on the canvas: `data-view="custom"`. Click "Front view"; Shift+right-drag 100 pixels: `data-view` stays `front` and `data-camera-target` changes. Right-click `(cx, cy)` without moving: `viewport-menu` is visible, `data-selection-count="1"` and `data-view="front"`; Escape closes it.
66. E-66. Cold start: the toolbar button "Extrude" has `aria-disabled="true"`. Click it: an info `notification` reads "Draw a closed shape in a sketch first." and no `inspector` opens. Click the canvas at `(cx, cy)` and press F: an info `notification` reads "Make a solid first, for example with Extrude or Box."
67. E-67. Drop `rect-sketch.ptsm`, double-click "Sketch 1", click "Finish sketch": `data-context="model"`, `selection-title` "1 region", and the inspector has a button "Extrude". Click it: the inspector "Extrude" whose profile `selection-field` has `data-count="1"`.
68. E-68. Drop `hole-cube.ptsm`, double-click "Sketch 2", press ControlOrMeta+Enter: `selection-title` "1 point" and a button "Hole". Press H: the inspector "Hole" whose points `selection-field` has `data-count="1"`; Enter: "Hole 1" has `data-status="ok"`.
69. E-69. Drop `cube.ptsm`, top fit, click `(cx, cy)`: `selection-title` "1 face" and a button "Hole". Click it: the inspector "Hole" whose points `selection-field` has `data-active="true"` and reads "Click a flat face or pick sketch points". Click `(cx, cy)`: `data-count="1"`. Press Enter: timeline items "Sketch 1", "Extrude 1", "Sketch 2", "Hole 1", all `data-status="ok"`; hover `(cx, cy)` gives `none` (Through all is the start value, 2.12). ControlOrMeta+Z: two timeline items.
70. E-70. Set the viewport to 1040 by 720 (the narrowest window where the browser starts open and docked, 3.2). New sketch on "Top plane (XY)" (as E-12): the button "Finish sketch" shows the text "Finish sketch"; `sketch-hint` reads "Choose Line (L), Rectangle (R) or Circle (C) to start drawing.". Press L: "Click to start a line.". Click `(cx, cy)` and move to `(cx + 150, cy)`: the hint contains "type a length" and the `draw-input` named "Length" has a placeholder matching `^\d+(\.\d+)?$`. Press Escape three times: `sketch-hint` reads "To leave the sketch, click Finish sketch or press Ctrl+Enter." and `data-context` is still "sketch".
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
97. E-97. Drop `cube.ptsm`, choose the radio "Print": the toolbar button is named "Printer: Generic FDM 0.4 mm (default)" (its label is its accessible name, shown as text only when the labels fit, 3.2) and `status-printer` "Generic FDM 0.4 mm (default)"; open it: the item "Generic FDM 0.4 mm" contains "256 × 256 × 256 mm bed · 0.4 mm nozzle".
98. E-98. Drop `cube.ptsm`, front fit. Right-click the top front edge `(cx, cy - 0.4 H)`: `viewport-menu` has items "Fillet", "Chamfer" and "Edit Extrude 1". Choose "Chamfer", press Enter: "Chamfer 1" has `data-status="ok"`. Click the right front edge `(cx + 0.4 H, cy)` and press Enter: the inspector "Chamfer" whose edges field has `data-count="1"` and whose Distance input has the value "1 mm"; Enter: "Chamfer 2" has `data-status="ok"`.
99. E-99. Drop `cube.ptsm`, top fit, press F: the edges field reads "Select edges or a face". Click `(cx, cy)`: `data-count="4"`. Click again: `data-count="0"`. Click again and press Enter: "Fillet 1" has `data-status="ok"`.
100. E-100. Drop `cube.ptsm`, choose "Print" and wait for `health-badge` `data-stale="false"`. Click "Lay on face": `print-hint` reads "Pick the face that should touch the bed.". `fitView(page, 'Front')`, click `(cx, cy)`: "Lay on face" has `aria-pressed="true"` and `health-badge` reaches `data-stale="false"` again. Click "Export" in the top bar: `export-status` contains "Printing with the picked face of Cube on the bed.". Escape, click "Lay on face": `aria-pressed="false"`.
101. E-101. Press F1: `keys-panel` is visible and its "Orbit" row contains "Shift + middle drag". Click "FreeCAD" in it: `data-nav-preset="freecad"` and the "Orbit" row contains "Middle + left drag".
102. E-102. Drop `rect-sketch.ptsm`, double-click "Sketch 1", press Escape five times: `data-context` is still "sketch".
103. E-103. Drop `rect-sketch.ptsm`, double-click "Sketch 1"; with `s = sketchToScreen` for content (0, 0) to (40, 20), click the top line at `(cx, cy - 10 s)` and press H: an info `notification` reads "Horizontal is already implied by other constraints, so it was not added." and the number of `constraint-glyph[data-type=horizontal]` is unchanged.
104. E-104. Cold start, click "Sketch" in `empty-state`, then "Sketch on Top plane (XY)": `data-view="top"`. Press R, click `(cx, cy)`, click `(cx + 100, cy + 60)`, press ControlOrMeta+Enter: `data-view="iso"` and `selection-title` "1 region". Double-click the timeline item "Sketch 1", right-drag 100 pixels upward, click "Finish sketch": `data-view="custom"`.

Layout (boxes measured in the browser, which jsdom cannot do):

105. E-105. At UI scale 1 and at UI scale 2 (stored in the settings before the first load, 2.15; the root font size is then 16 and 32 CSS pixels, 2.16.9), one test each: in the iso view the 8 corner buttons of the view cube (named "... corner view", 3.14) lie inside the `viewport` box, up to 0.5 pixel of rounding.
106. E-106. Drop `cube.ptsm`, press E: every segmented choice (`radiogroup`) and select of the inspector lies inside its `inspector-field` box (3.7). Check "Intersect", "Cut", "Join" and "New body" of Operation in turn: each is checked and every choice still lies inside its field.
107. E-107. At UI scale 2, click "Sketch" in `empty-state`: `sketch-header` has `data-state="picking"`, and the header, its prompt "Pick a plane or a flat face to draw on" and its buttons "Sketch on Top plane (XY)", "Sketch on Front plane (XZ)" and "Sketch on Right plane (YZ)" lie inside the `viewport` box (3.16.1).

Fixtures:

108. E-108. Each fixture of 6.2 with a stated result opens with it, one test per fixture (`notes.txt` is E-5; the perf fixtures state none). Drop it, then: `cube.ptsm`: timeline "Sketch 1", "Extrude 1", each `data-status="ok"`; clicking the body row "Cube" shows `body-size` "20 × 20 × 20 mm" and `body-center` "0, 0, 10 mm". `rect-sketch.ptsm`: timeline "Sketch 1" (ok); double-click it: `data-dof="0"` and `data-state="full"`; ControlOrMeta+Enter: `selection-title` "1 region"; press E: the profile field has `data-count="1"`; fill Distance with "10", Enter: "Extrude 1" ok, and "Body 1" shows "40 × 20 × 10 mm" and "20, 10, 5 mm". `underconstrained.ptsm`: timeline "Sketch 1" (ok); double-click it: `data-dof="7"`. `parametric.ptsm`: as `cube.ptsm`; run "Parameters": the row with `data-id="p-size"` is named "size" with `parameter-value` "20 mm". `cube-features.ptsm`: timeline "Sketch 1", "Extrude 1", "Fillet 1", "Shell 1", each ok. `two-bodies.ptsm`: timeline "Sketch 1", "Extrude 1", "Cylinder 1", each ok. `fillet-error.ptsm`: three items, "Extrude 1" ok and "Fillet 1" `data-status="error"`; double-click "Fillet 1": the radius field's `field-error` contains "Fillet radius 15 mm is too large" (`fillet_too_large` on `/radius`) and `inspector-error` has a button whose name starts with "Use " (the suggestion). `hole-cube.ptsm`: timeline "Sketch 1", "Extrude 1", "Sketch 2", each ok. `revolve-sketch.ptsm`: timeline "Sketch 1" (ok) and the browser button "Hide Sketch 1"; double-click it: `data-dof="0"`; ControlOrMeta+Enter: `selection-title` "1 region". `offset-cube.ptsm`: as `cube.ptsm` with `body-center` "20, 0, 10 mm". `overhang.ptsm`: choose the radio "Print", wait for `health-badge` `data-stale="false"`, click "Print check": at least one `health-item[data-check=overhangs]`. `oversize.ptsm`: the same with at least one `health-item[data-check=bed_fit][data-severity=error]`. `hollow-resin.ptsm`: the same with at least one `health-item[data-check=trapped_volumes][data-severity=warning]`.

Lost references, fixes and newcomer errors with the real core (the reference stability study section 6.3, U14). E-110 to E-121 are the newcomer error corpus's scenarios NC01 to NC12, with the corpus's texts; the study numbered its other e2e tests from E-105, which 6.5 already used, so its E-105 to E-108 are E-123 to E-126 here, and its E-109 and E-122 keep their numbers. Texts are exact; "the Problems row" of a feature is its `problem-row` in the Problems panel, opened with `timeline-problems` (or the palette command "Problems" when it is hidden).

109. E-109. Corpus R03 in a sketch session. Drop `bevel-corner.ptsm`, double-click "Sketch 1", and with `s = sketchToScreen` for content (0, 0) to (80, 50) click the bevel line at (75, 45): `selection-title` "1 line", and the `entity-used` mark with `data-entity="e-5"` has the accessible name "Used by 'Fillet 1'". Press Delete: `sketch-hint` reads "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it." and no notification appears. Press ControlOrMeta+Enter: exactly one `notification`, `data-severity="warning"`, reading "'Fillet 1' lost the top edge of 'Extrude 1' along a line of 'Sketch 1'.", whose `cause-line` reads "This started after you deleted the line from (80, 40) to (70, 50) in 'Sketch 1'." and whose buttons are "Show what 'Fillet 1' lost" and "Undo"; "Extrude 1" is ok and "Fillet 1" warning. The Problems row of 'Fillet 1' holds "The top edge of 'Extrude 1' along a line of 'Sketch 1' selected for Edges is gone because that line was deleted from 'Sketch 1', so it was skipped.", buttons starting with "Use ", then "Remove from Edges" and "Pick an edge again", and the cause line with "Undo" and "Show the line". Click "Show the line": one `cause-ghost` is visible. Click "Remove from Edges": "Fillet 1" is ok.
110. E-110. NC01, open outline. Drop `nc01.ptsm`: the toolbar button "Extrude" has `aria-disabled="true"`. Click it: an info `notification` reads "'Sketch 1' has no closed shape: 2 ends are not connected." with a button "Show 'Sketch 1'". Click it: `data-context="sketch"`, and `sketch-header` has `data-outline="2"` and contains "2 open ends". Click the header: `sketch-problems` contains "The end of the line from (0, 50) to (0, 0.3) at (0, 0.3) is not connected to other geometry." and a button "Connect the ends (0.3 mm apart)". Click it: `data-outline="0"`, and `sketch-problems` stays open without that sentence (3.16.8). Press Escape (it hides), then ControlOrMeta+Enter: `selection-title` "1 region", and "Extrude" has no `aria-disabled="true"`.
111. E-111. NC02, join buried in the body. Drop `nc02.ptsm`: "Extrude 2" has `data-status="warning"`; its Problems row reads "This extrude adds nothing to body 'Body 1': it lies entirely inside the body." with the button "Attach 'Sketch 2' to the top of 'Extrude 1'". Click it: "Extrude 2" is ok, and the body row "Body 1" shows `body-size` "80 × 50 × 40 mm". ControlOrMeta+Z: "Extrude 2" has `data-status="warning"` again.
112. E-112. NC03, cut that points away. Drop `nc03.ptsm`, double-click "Extrude 2", fill Distance with "5", and wait for `data-preview="shown"`: `inspector-error` contains "This cut does not touch body 'Body 1': it starts 10 mm from the body and points away from it." and has no button "Turn on Reverse" (reversed, the cut misses too). Escape. The Problems row of 'Extrude 2' reads the same sentence with the button "Turn on Reverse". Click it: "Extrude 2" is ok and 'Body 1' keeps `body-size` "80 × 50 × 30 mm".
113. E-113. NC04, floating join. Drop `nc04.ptsm`: the Problems row of 'Extrude 2' reads "This extrude does not touch body 'Body 1', so the body now has 2 separate pieces." with the buttons "Put it in a new body" and "Attach 'Sketch 2' to the top of 'Extrude 1'", in that order. Click the second: "Extrude 2" is ok and 'Body 1' shows `body-size` "80 × 50 × 45 mm".
114. E-114. NC05, fillet too large. Drop `nc05.ptsm` and double-click "Fillet 1": the radius field's `field-error` reads "Fillet radius 3 mm is too large for the selected edges: the largest that works is about 2.3 mm. The top edge of the cut made by 'Box 2' lies next to a wall of the cut made by 'Box 2', which is only 2.4 mm across.", its `field-limit` reads "Largest that works: 2.3 mm", and `inspector-error` has the button "Use 2.3 mm". Click it, wait for `data-preview="shown"`, press Enter: "Fillet 1" is ok; double-click it: the radius input has the value "2.3 mm".
115. E-115. NC06, 2 mm walls after a 1 mm fillet. Drop `nc06.ptsm`: "Box 1", "Fillet 1" and "Shell 1" have `data-status="ok"`, Problems has no row for 'Shell 1', and clicking the body row "Body 1" shows `body-size` "40 × 40 × 20 mm" (the walls meet in sharp inner corners, kernel.md section 3.4.8 item 6; newcomer error corpus NC06).
116. E-116. NC07, constraint conflict. Drop `nc07.ptsm`: "Sketch 1" and "Extrude 1" have `data-status="error"`, and `timeline-problems` reads "1 problem". The Problems panel has one top-level row, 'Sketch 1', reading "In 'Sketch 1', Distance 30 mm cannot hold together with Distance 40 mm." with the buttons "Delete Distance 30 mm", "Make Distance 30 mm a reference", "Delete Distance 40 mm" and "Make Distance 40 mm a reference", in that order, and under it the row of 'Extrude 1' with `data-nested="true"` and the button "Show 'Sketch 1'". Click "Delete Distance 30 mm": both are ok, and 'Body 1' shows `body-size` "40 × 20 × 10 mm".
117. E-117. NC08, a value the sketch cannot take. Drop `nc08.ptsm`, double-click "Sketch 1", double-click the dimension label "50", fill "80", Enter: `dimension-editor` stays open with a `role=alert` reading "'Sketch 1' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape." and the buttons "Add reference" and "Cancel", and the label still reads "50". Then drop `nc08-broken.ptsm`: "Sketch 1" has `data-status="error"`, and its Problems row reads the same sentence with no `cause-line` (a document opened fresh has no cause).
118. E-118. NC09, units switch with bare numbers. Without File System Access, drop `nc09.ptsm` and click `status-units`: the `keep-sizes` item is checked and reads "Keep sizes: add mm to the 4 numbers without a unit". Choose "in": `status-units` reads "in" and "Fillet 1" is ok. Press ControlOrMeta+S: the download contains `"40 mm"`, `"20 mm"`, `"10 mm"` and `"2 mm"`. ControlOrMeta+Z: `status-units` reads "mm".
119. E-119. NC10, misspelled parameter. Drop `nc10.ptsm`: the Problems row of 'Extrude 1' reads "Distance: 'hieght' is not a parameter. Did you mean 'height'?" with the button "Replace hieght with height". Click it: "Extrude 1" is ok.
120. E-120. NC11, a deleted sketch line, and what uses it. Drop `nc11.ptsm`. Right-click "Extrude 1": the context menu has the item "Used by 'Fillet 1'"; Escape. Double-click "Sketch 1": exactly one `entity-used` mark, with `data-entity="e-3"`. With `s = sketchToScreen` for content (0, 0) to (80, 50), click the back line at (40, 50) and press Delete: `sketch-hint` reads "'Fillet 1' uses the edge made from this line. Draw its replacement in the same place and 'Fillet 1' will follow it.". Press ControlOrMeta+Enter: besides the sketch's `profile_problems` warning (3.16.11, the outline is now open), exactly one `notification` with `data-severity="error"`, reading "'Extrude 1' lost the region of 'Sketch 1' around (40, 25), so 1 feature after it failed too.", whose `cause-line` reads "This started after you deleted the line from (80, 50) to (0, 50) in 'Sketch 1'." and whose buttons are "Show what 'Extrude 1' lost" and "Undo". The Problems row of 'Extrude 1' reads "The region of 'Sketch 1' selected for Profile is gone because part of its outline was deleted." with "Show 'Sketch 1'" and "Pick a region again", and under it 'Fillet 1' (`data-nested="true"`) with "Show 'Extrude 1'". Click the notification's "Undo": every feature is ok and no error notification remains.
121. E-121. NC12, hole beside the body. Drop `nc12.ptsm`: the Problems row of 'Hole 1' reads "The hole at (95, 25) in 'Sketch 2' does not touch body 'Body 1'. Move the point onto the body." with no button "Turn on Reverse".
122. E-122. A fix that no longer applies. Drop `pattern-fillet.ptsm`, double-click "Linear pattern 1", fill Count with "2", Enter: the loss notification of E-124 appears. With the pointer over it, run "Units: cm" from the palette (an unrelated edit), then click its "Set Count of 'Linear pattern 1' back to 3": an error `notification` with `data-code="stale_fix"` reads "This fix no longer applies because the document changed after it was offered. Look at the problem again." with the button "Show problems"; double-click "Linear pattern 1": Count still reads "2".
123. E-123. Corpus R16, a cut that removes the filleted edge. Drop `rabbet.ptsm`, right-click "Extrude 1" and choose "Unsuppress": "Fillet 1" has `data-status="error"`, and exactly one `notification`, `data-severity="error"`, reads "'Fillet 1' lost the front top edge of 'Box 1'.", with the `cause-line` "This started after you unsuppressed 'Extrude 1'." and the buttons "Show what 'Fillet 1' lost", "Suppress 'Extrude 1'" and "Undo". Click "Show what 'Fillet 1' lost": the inspector "Fillet 1" in `data-mode="command"`, one `repair-ghost`, at least two `repair-candidate` markers, and the buttons "1. The front edge of the cut made by 'Extrude 1'" and "2. The top edge of the cut made by 'Extrude 1'". Click button 1, wait for `data-preview="shown"`, press Enter: "Fillet 1" is ok.
124. E-124. Corpus R41, a pattern count lowered. Drop `pattern-fillet.ptsm`, double-click "Linear pattern 1", fill Count with "2", Enter: "Fillet 1" has `data-status="warning"`, and exactly one `notification`, `data-severity="warning"`, reads "'Fillet 1' lost the right top edge of 'Box 1' of the third copy of 'Linear pattern 1'.", with the `cause-line` "This started after you changed Count of 'Linear pattern 1' from 3 to 2." and the buttons "Show what 'Fillet 1' lost", "Set Count of 'Linear pattern 1' back to 3" and "Undo". Click "Set Count of 'Linear pattern 1' back to 3": "Fillet 1" is ok and the notification is gone; ControlOrMeta+Z: "Fillet 1" has `data-status="warning"` again.
125. E-125. Corpus R28, a fillet dragged above what makes its edge. Without File System Access, drop `boss-fillet.ptsm` and drag "Fillet 1" over the gap between "Extrude 1" and "Datum plane 1": `drop-hint` reads "'Fillet 1' uses an edge made by 'Extrude 2'. Drop below 'Extrude 2' instead?". Drop there: "Fillet 1" has `data-status="error"`, and the Problems row of 'Fillet 1' has the button "Move 'Fillet 1' below 'Extrude 2'". Click it: every feature is ok; press ControlOrMeta+S: the download's bytes equal the fixture's bytes.
126. E-126. Corpus R06 (a), a side line redrawn. Drop `side-line.ptsm`, double-click "Sketch 1", and with `s = sketchToScreen` for content (0, 0) to (80, 50) click the right line at (80, 25), press Delete, press L, click (80, 0) and (80, 50) (both snap to the remaining ends), press Escape, and press ControlOrMeta+Enter: no notification appears, "Fillet 1" has `data-status="ok"` and `data-notes="1"`, and `timeline-problems` is hidden. Open the Problems panel with the palette command "Problems" and open `problems-notes`: one note of 'Fillet 1' with the button "Keep this match". Click it: `data-notes="0"` and no note remains.

Recovery copies and a core that is still working (3.19, 4.4):

127. E-127. Replaced, then autosave, then save. `openApp(page, {fileSystemAccess: false})`, drop `cube.ptsm`, rename the body "Cube" to "Box" (click its row, press F2, type, Enter) and wait 3 s. Drop `two-bodies.ptsm`: a notification reads "cube had unsaved changes. They are kept for you." Rename the body "Peg" to "Pin", wait 3 s and press ControlOrMeta+S: the download `two-bodies.ptsm` contains `"name": "Pin"`, and `data-dirty="false"`. Reload: exactly one notification starts with "Partsmith kept unsaved changes", it starts with "Partsmith kept unsaved changes to cube from" and does not contain "other document"; click "Open": `document-name` "cube", a body row "Box" and `data-dirty="true"`.
128. E-128. Two tabs, in one browser context (so one IndexedDB and one lock manager). In tab A drop `cube.ptsm`, rename the body to "Box" and wait 3 s. Open tab B (`context.newPage()`, then `openApp`): no notification starts with "Partsmith kept unsaved changes", because A's copy is live. In B drop `two-bodies.ptsm`, rename "Peg" to "Pin" and wait 3 s. Close tab A (`page.close()`, which runs no `beforeunload`) and open tab C: exactly one notification starts with "Partsmith kept unsaved changes", it starts with "Partsmith kept unsaved changes to cube from" and does not contain "other document"; click "Discard". Close tab B (again without `beforeunload`) and open tab D: exactly one such notification, starting with "Partsmith kept unsaved changes to two-bodies from"; click "Open": body rows "Cube" and "Pin", and `data-dirty="true"`.
129. E-129. A stuck core (test timeout 60 s). Call `stallCore(page, 'Stall here')`, then `openApp`. Drop `cube.ptsm`, rename the body to "Box" and wait 3 s. Rename it to "Stall here" (the core never answers) and click "Hide Box" in its row, so a change waits behind the rename. `status-core` reads "Rebuilding" within 1 s. 8 s after the second rename `core-working` is not shown; within 14 s of it, it is, with the `core_still_working` text for 10 seconds and the button "Stop and restart core", and `status-core` reads "Still working". Click the button: `core-working` disappears, `data-core-status="ready"` returns with no `core-fatal`, `document-name` is "cube", the body row reads "Box" with the button "Hide Box", `data-dirty="true"`, and a `notification[data-severity=warning]` has the `changes_not_applied` text. Rename "Box" to "Lid" (the new core answers) and wait 3 s. Reload, accepting the `beforeunload` dialog: exactly one notification starts with "Partsmith kept unsaved changes", and it starts with "Partsmith kept unsaved changes to cube from" (the restart continued the session's entry); click "Open": a body row "Lid".

A core that is still working, continued (3.17, 4.4):

130. E-130. A slow print check is not a stuck core (test timeout 60 s). Call `stallCore(page, 'analyze_print')` (only the print analysis request contains it), then `openApp`. Drop `cube.ptsm` and choose the radio "Print": `health-badge` has `data-state="running"` and reads "Checking". 14 s later `core-working` and `status-core` are not shown, `health-badge` still has `data-state="running"`, and `viewport`, the toolbar "Print tools" and the radiogroup "Context" are visible. Rename the body "Cube" to "Box" (click its row, press F2, type, Enter), which the core takes only after the stuck analysis: `status-core` reads "Rebuilding" within 1 s; 8 s after the rename `core-working` is not shown, and within 14 s of it, it is, with the button "Stop and restart core". Click it: `data-core-status="ready"` returns, the body row reads "Cube", and a `notification[data-severity=warning]` has the `changes_not_applied` text.
131. E-131. The still-working panel leaves the model and the window usable (test timeout 60 s). Call `stallCore(page, 'Stall here')`, then `openApp`. Drop `cube.ptsm`, choose the radio "Print" and wait for `health-badge` `data-stale="false"`. Rename the body to "Stall here": within 14 s `core-working` shows inside `viewport`. While it shows, `viewport` and its canvas are visible, the toolbar "Print tools" has the buttons "Overhang map" and "Lay on face", and the radiogroup "Context" has "Print" checked. "Overhang map" has `aria-pressed="true"`; click it: `aria-pressed="false"`. Right-drag 100 pixels on the canvas from `(cx, cy + 0.4 H)`, below the panel: `data-view="custom"`. Choose the radio "Model": `data-context="model"` and the toolbar "Model tools" is visible, with `core-working` still shown. The inspector shows the renamed body (`data-mode="selection"`): drag it by its header label "Selection" until the label's center is 40 pixels above `core-working`, at the horizontal center of the button "Stop and restart core": the inspector's box contains the button's center, and `document.elementFromPoint` at that center is the button or inside it. Click "Stop and restart core": `core-working` disappears and `data-core-status="ready"` returns.

Tooltips after Esc (3.24):

132. E-132. Esc gives focus back without a tooltip. Drop `cube.ptsm` and click "Export" in the top bar: `export-popover` is visible. Press Escape: `export-popover` is hidden, "Export" has focus, no `[role=tooltip]` is visible, `top-bar` is visible, and no element reads "This part of the window hit a problem. Reload to restore it." Press Shift+Tab, then Tab: "Export" has focus and a visible `[role=tooltip]` contains "Export". `fitView(page, 'Front')` and press Shift+X: `section-popover` is visible. Press Escape: `section-popover` is hidden, "Fit to view" has focus, no `[role=tooltip]` is visible, and `viewport` is visible. (Chromium 156, which CI runs, throws on a tooltip shown while it closes a popover; without the rule of 3.24 the region crashes there, as E-45, E-50, E-95 and E-100 did on the first CI run, while older Chromium passes either way.)

Islands, the free area and the axes (3.2, 3.7, 3.8, 3.13, 3.14, 3.24):

133. E-133. Islands at 1280 by 720 and 100 percent: after `openApp` the canvas element's bounding box is (0, 0, 1280, 720); `viewport` is (280, 64, 992, 548); `top-bar` (8, 8, 1264, 48), `browser` (8, 64, 264, 548), `timeline` (8, 620, 1264, 56) and `status-bar` (8, 684, 1264, 28), each within 0.5 pixel; these four have a computed `border-top-left-radius` of `12px` (`status-bar` `8px`) and a computed `box-shadow` other than `none`. Run "Parameters": `side-panel` is (912, 64, 360, 548) and `viewport` (280, 64, 624, 548). Click "Toggle browser": `viewport` is (8, 64, 896, 548). Click "Collapse timeline": `timeline` is (8, 648, 1264, 28) with a `border-top-left-radius` of `8px`, and `viewport` is (8, 64, 896, 576).
134. E-134. Framing in the free area: drop `cube.ptsm`, `fitView(page, 'Front')`: hover `(cx, cy)` gives `face`, `(cx + 0.4 H - 10, cy)` gives `face` and `(cx + 0.4 H + 12, cy)` gives `none`. Click "Toggle browser" and read `canvas(page)` again: `cx` is 136 pixels smaller and `H` unchanged, the same three hovers relative to the new center give `face`, `face` and `none` (the model moved to the new center at the same size), and `data-view` is still `front`. Click "Fit to view": the same three hovers give the same kinds.
135. E-135. Nothing is picked through an island: drop `cube.ptsm`, `fitView(page, 'Front')`, then ten wheel events of `deltaY: -100` at `(cx, cy)`, so the front face covers the whole window. Hover `(cx, cy)`: `face`. Move to 20 pixels left of `browser`'s right edge at the height `cy`: `data-hover-kind="none"`; a wheel event of `deltaY: 100` there leaves `data-camera-target` unchanged. Move to 4 pixels right of `browser`'s right edge (the gap between the browser and the free area): `face`. Move to the middle of `top-bar`'s bottom edge, 4 pixels inside it: `none`.
136. E-136. Corners follow the free area: drop `cube.ptsm`: the right edge of `view-cube`'s box is 36 pixels left of `viewport`'s right edge and its top 44 pixels below `viewport`'s top, and the left edge of `viewport-controls` is 16 pixels right of `viewport`'s left edge and its top 16 pixels below `viewport`'s top (each within 0.5 pixel). Run "Parameters": the same holds in the narrower `viewport`, with the cube 368 pixels further left. Click "Toggle browser": the same holds, with the controls 272 pixels further left.
137. E-137. The top-center slot clears the cube and the controls: (a) at 1040 by 720, run "Parameters" (the browser and the side panel docked, `viewport` 384 pixels wide) and click "Sketch" in `empty-state`: `sketch-header` has `data-state="picking"`, its box lies inside `viewport`, its top is at least 192 pixels below `viewport`'s top, and it intersects neither `viewport-controls`, any of the 8 view cube corner buttons (iso view), nor "Home view", and wherever it is level with "Home view" it ends at least 0.5rem (8 pixels times the UI scale) left of it. (b) At 1280 by 720 with the Parameters panel open, the same: the header's top is 16 pixels below `viewport`'s top and it intersects neither. (c) At UI scale 2 (stored in the settings before the first load, as E-105) on 1280 by 720, without the Parameters panel (at 200 percent it floats over the cube and closes on the click): as (b), with a top 32 pixels below. (d) At UI scale 2 on 2080 by 720 with the Parameters panel open (both docked, `viewport` 768 by 376 pixels, too short for the 12rem drop), click "Sketch": `sketch-header` and its plane buttons lie inside `viewport`.
138. E-138. Overlay islands at 200 percent: at UI scale 2 on 1280 by 720, `document-name` ends at or left of the context switcher's left edge and shows its whole text (its `scrollWidth` is at most its `clientWidth`), `browser` is hidden and `viewport` is (16, 128, 1248, 376) within 0.5 pixel. Click "Toggle browser": `browser` is visible, its left edge is at `viewport`'s left edge, its box lies inside `viewport`'s, and `viewport` is unchanged. Press Escape: `browser` is hidden. Run "Parameters": `side-panel` lies inside `viewport` and is 720 pixels wide; press Escape: it is hidden. The 8 view cube corner buttons (iso view), "Home view" and the buttons of `viewport-controls` lie inside `viewport`, and no control button intersects a corner button. Drop `cube.ptsm`, choose "Print" and wait for `health-badge` `data-stale="false"`: `overhang-legend` lies inside `viewport` and intersects neither `viewport-controls`, `timeline` nor a view cube corner button.
139. E-139. Axis letters in fields: drop `cube.ptsm` and run "Move body" from the palette: in the inspector "Move body" the fields of "Move X", "Move Y" and "Move Z" each hold an `axis-letter` reading X, Y and Z with `data-world="true"`, whose box lies inside its input's box within 8 pixels of the input's left edge, and each input's computed `padding-left` is `24px`. Click "Cancel", open "Shapes" and click "Box", then click "More options": the letters of "Position X", "Position Y" and "Position Z" have `data-world="true"`; choose "Front plane (XZ)" in the Plane field's Choose menu: they have `data-world="false"`, and the inputs keep the names "Position X", "Position Y" and "Position Z".
140. E-140. Coordinate readouts: drop `cube.ptsm` and click the body row "Cube": `body-size` reads "X 20 Y 20 Z 20 mm" with `data-value` "20 × 20 × 20 mm", and `body-center` reads "X 0 Y 0 Z 10 mm" with `data-value` "0, 0, 10 mm"; the X letters of both have the same left edge within 0.5 pixel, as do their Y and their Z letters. Press Escape (the selection clears and the inspector closes, so the fit frames the whole free area, 2.11), `fitView(page, 'Front')` and click the vertex at `(cx + 0.4 H, cy - 0.4 H)`: `vertex-position` reads "X 10 Y -10 Z 20 mm" with `data-value` "10, -10, 20 mm". Click the body row "Cube", run "Move body" from the palette, type `-1234.56` into "Move X", "Move Y" and "Move Z", click "OK" and click the body row "Cube" again: `body-center` reads "X -1234.56 Y -1234.56 Z -1224.56 mm", and its box lies inside `inspector`'s box.
141. E-141. The viewport menu opens at the pointer: drop `cube.ptsm`, `fitView(page, 'Front')`, and send a wheel event of `deltaY: 100` at `(cx + 200, cy)` (the target moves). Right-click at `(cx + 250, cy + 50)`, beside the model, so nothing is selected and no inspector opens (an open inspector would frame beside it, 2.11): `viewport-menu` is visible and its top-left corner lies within 4 pixels of that point. Click "Fit to view" in it: `data-camera-target` is "0,0,10".
142. E-142. The inspector comes back into the free area: drop `cube.ptsm` and click the body row "Cube"; drag the inspector's header 2000 pixels to the right: its right edge is at 1272. Run "Parameters": once the side panel is visible, the inspector's box lies inside `viewport`'s, and `document.elementFromPoint` at the center of its "Clear selection" button lies in `inspector`.

Type and self-hosted fonts (2.16.1, 2.16.7, ADR 0031):

143. E-143. Interface text keeps `ss05` and numbers in fields are tabular: drop `cube.ptsm` and double-click "Extrude 1". The computed `font-feature-settings` of `document-name`, of the `inspector-field` label "Distance", of the input "Distance", of the button "OK" and of `status-units` contains `ss05`. The computed `font-variant-numeric` of the input "Distance", of `expression-result` and of `status-recompute` is `tabular-nums`, and that of `document-name`, of the label "Distance" and of the button "OK" is `normal`. Click "Cancel" and click the body row "Cube": each number of `body-size` has a `font-feature-settings` containing `ss05` and the `font-variant-numeric` `tabular-nums`, and `selection-title` has `normal`. In both states, every element of the page whose first family (as in E-62) is `Geist` has a `font-feature-settings` containing `ss05`, and no element has a `font-variant-numeric` containing `slashed-zero`.
144. E-144. No font request leaves the app: record every request with `page.on('request')` from before `openApp`, with its URL, its `resourceType()` and whether it sent an `Origin` header (`request.headers()`), so a failure shows the mode of each request it counted (2.16.7). After `openApp` and `document.fonts.ready`, `document.fonts.check('14px Geist')` is true, exactly one request has the path `/fonts/Geist-Variable.woff2` (the preload, which the face then uses) and none has the path `/fonts/JetBrainsMono-Variable.woff2`. Drop `parametric.ptsm`, run "Parameters" and wait until `document.fonts.check('14px "JetBrains Mono"')` is true: exactly one request has the path `/fonts/JetBrainsMono-Variable.woff2`. Every recorded request whose `resourceType()` is `font` has the page's origin and a path under `/fonts/`, and no recorded request has the host `fonts.googleapis.com` or `fonts.gstatic.com`.

Flat islands (3.2, 3.23, 3.24, ADR 0032). E-145 and E-146 drop `cube.ptsm`, run "Parameters", click the browser row "Cube" (the selection inspector opens) and then click `document-name`, so the Document menu is open; "the eight surfaces" are `top-bar`, `browser`, `side-panel`, `timeline`, `status-bar`, `inspector`, `viewport-controls` and `document-menu`, and "the switcher's labels" are the `label` elements around the context switcher's radios "Model" (checked), "Sketch" and "Print".

145. E-145. No outlines, in each theme (`page.emulateMedia({ colorScheme })` before `openApp`): each of the eight surfaces has a computed `border-top-width` of `1px` and a `border-top-color` of `rgba(0, 0, 0, 0)`, and `palette-button`, a ghost button, a `border-top-width` of `0px`; each of the eight surfaces has a computed `box-shadow` other than `none`; the seven other than `document-menu` have the same computed `background-color`, which differs from that of `app`, and `document-menu`'s equals theirs in the light theme and differs from theirs and from `app`'s in the dark theme. Of the switcher's labels, "Model" has `outline-style` `solid` with `outline-color` `rgba(0, 0, 0, 0)` and a `background-color` that differs from the radiogroup's, and "Sketch" and "Print" have `outline-style` `none`, `outline-color` `rgba(0, 0, 0, 0)` and `background-color` `rgba(0, 0, 0, 0)`. The browser row "Cube" has `outline-style` `solid` with `outline-color` `rgba(0, 0, 0, 0)`.
146. E-146. Edges in forced colors, with `page.emulateMedia({ forcedColors: 'active', colorScheme })` before `openApp`, for `light` and for `dark`: each of the eight surfaces has a computed `box-shadow` of `none`, a `border-top-style` of `solid`, a `border-top-width` of `1px` and a `border-top-color` that is not `rgba(0, 0, 0, 0)`, equals the element's computed `color` and differs from its computed `background-color`, and `palette-button` keeps a `border-top-width` of `0px`, as ghost buttons draw no edge. Of the switcher's labels, "Model" has `outline-style` `solid`, `outline-width` `1px` and an `outline-color` equal to its computed `color`, and "Sketch" and "Print" `outline-style` `none`; the browser row "Cube" (selected) has `outline-style` `solid` and an `outline-color` equal to its `color`. Press Escape and click "Section view" in `viewport-controls`: it has `aria-pressed="true"`, `outline-style` `solid` and an `outline-color` equal to its `color`, and the thumb of the switch "Flip" in the section popover (the `span` inside it) has a `border-top-width` of `1px` and a `border-top-color` equal to its `color`. Press Escape and click the color button of the row "Cube": each swatch of `color-popover` has the computed `background-color` of its step's `PALETTE_HEX` value (the Default swatch `body`'s `zinc-400`). Press Escape, then E: in the Extrude inspector that opens, the input of the Distance field keeps a `border-top-width` of `1px` and a `border-top-color` equal to its computed `color`.
147. E-147. The backend clears to the viewport's color, in `ui/e2e/viewport.spec.ts` on the harness page of `e2e/pages/viewport.html`: the harness renders a frame with no draws once with `viewportTheme('light')` and once with `viewportTheme('dark')` as its theme, and the pixels read back at the drawing buffer's center and four corners are (244, 244, 245, 255) and (9, 9, 11, 255), within 1 per channel, so the background is written as sRGB (a linear value passed through unencoded, for example to `gl.clearColor`, would read about (231, 231, 233) and (1, 1, 1)). The harness listens for `webglcontextcreationerror` on the canvas before it creates the backend, and a canvas without a WebGL2 context fails the test with "createWebGl2Backend returned null: this browser gave the canvas no WebGL2 context ({reason})." (`{reason}` the event's `statusMessage`, or "no reason given" without an event or with an empty message), which names the graphics path that failed in whichever engine runs it (6.8).
148. E-148. Overlay islands are raised over the free area's cards, in the dark theme (`page.emulateMedia({ colorScheme: 'dark' })` before `openApp`) at UI scale 2 on 1280 by 720 (stored in the settings before the first load, as E-105): `browser` is hidden; click "Toggle browser": `browser`, now an overlay island, has a computed `background-color` that draws as `#27272a` (`zinc-800`, `surfaceRaised`) on a canvas, and `viewport-controls` one that draws as `#18181b` (`zinc-900`, `surface`). Store UI scale 1 and reload: the docked `browser`'s draws as `#18181b`.
149. E-149. A submenu opens beside its item: click `document-name`, then the item "Theme" of `document-menu`: it has `aria-expanded="true"`, and the `menu` named "Theme" is visible with its top within 0.5 pixels of the item's top and its left edge at or right of the item's right edge, not at the top bar's height (3.2).

Assets and files changed elsewhere (3.19, 4.4, ADR 0037). These tests use `useFolders` and the origin private file system (6.3), the fixture `step-block.ptsm` and its asset `S`, read from `examples/import/assets/` (6.2):

150. E-150. Opening a document with its folder. `useFolders(page)`, then `openApp`. Write `work/step-block.ptsm` (the fixture), `work/assets/S` (the bytes of `S`) and `other/notes.txt` (`x`). Queue `work/step-block.ptsm` and press ControlOrMeta+O: `document-name` is "step-block", the timeline item "Import 1" has `data-status="error"`, and `notification[data-code=assets_folder_needed]` reads "The imported geometry of step-block.ptsm is missing, because the browser has not given Partsmith the folder that holds it and its assets folder. Choose the folder step-block.ptsm is in." with the button "Choose folder". Queue `other` and click it: the last of `pickerCalls` is `showDirectoryPicker` with `mode` `readwrite` and `startIn` `step-block.ptsm`, and `notification[data-code=assets_wrong_folder]` reads "other is not the folder that holds step-block.ptsm. Choose the folder step-block.ptsm is in." Queue `work` and click its "Choose folder": "Import 1" has `data-status="ok"`, no `assets_folder_needed` notification is shown, `data-dirty="false"`, and after a click on the body row "Body 1", `body-size` reads "10 × 20 × 30 mm".
151. E-151. Save as into another folder writes the assets first. Do E-150's steps up to "Import 1" being ok, and write `copy/notes.txt` (`x`). Queue `copy/step-block.ptsm` and press ControlOrMeta+Shift+S: `notification[data-code=save_folder_needed]` is shown with "Choose folder", and `copy` holds no `assets`. Queue `copy` and click "Choose folder": `copy/assets/S` holds the bytes of `S`, `folderNames(page, 'copy/assets')` is `[S]`, `copy/step-block.ptsm` holds exactly the fixture's bytes (the unchanged document saves as its canonical text), `folderNames(page, 'work/assets')` is still `[S]`, and `data-dirty="false"`. Rename the body "Body 1" to "Block" and press ControlOrMeta+S: no picker is called, `copy/step-block.ptsm` contains `"name": "Block"`, `work/step-block.ptsm` still holds the fixture's bytes, and `data-dirty="false"`. Reload, open `document-menu`, then its "Open recent" submenu, and click its first entry, "step-block": the body row reads "Block", "Import 1" has `data-status="ok"`, and `pickerCalls` is empty, since the recent entry kept the folder `copy`.
152. E-152. Save stops when the file changed elsewhere. `useFolders(page)`, then `openApp`. Write `work/cube.ptsm` (the fixture), queue it and press ControlOrMeta+O; rename the body "Cube" to "Box". Write `work/cube.ptsm` again with the fixture's text in which `"Cube"` is replaced by `"Lid"`, as another program would, and press ControlOrMeta+S: `notification[data-code=save_conflict]` reads "cube.ptsm was changed elsewhere after you opened or saved it, so Partsmith did not save over it." with the buttons "Overwrite cube.ptsm", "Save as" and "Open the newer file", in that order, the file still contains `"name": "Lid"`, and `data-dirty="true"`. Click "Overwrite cube.ptsm": the file contains `"name": "Box"` and `data-dirty="false"`. Rename "Box" to "Peg", write the file's own bytes back unchanged (`readFolderFile`, then `writeFolderFile`) and press ControlOrMeta+S: no `save_conflict` is shown and the file contains `"name": "Peg"`. Rename "Peg" to "Pin", write the "Lid" text again, press ControlOrMeta+S, and click "Open the newer file": the body row reads "Lid", `data-dirty="false"`, and a notification reads "cube had unsaved changes. They are kept for you." with "Reopen".
153. E-153. A core restart keeps the imported geometry (test timeout 60 s). Call `stallCore(page, 'Stall here')` and `useFolders(page)`, then `openApp`. Open `work/step-block.ptsm` and choose the folder `work` as in E-150, until "Import 1" is ok. Rename the body "Body 1" to "Stall here" (the core never answers); once `core-working` is shown, click "Stop and restart core": `data-core-status="ready"` returns with no `core-fatal`, "Import 1" has `data-status="ok"` with no picker called after the restart, and the body row "Body 1" (the rename was not applied) shows `body-size` "10 × 20 × 30 mm" when clicked.
154. E-154. Without its folder, the warning says why. Drop `step-block.ptsm`: "Import 1" has `data-status="error"`, and `notification[data-code=assets_without_folder]` reads "The imported geometry of step-block.ptsm is missing, because Partsmith has no file of it to find its assets folder by. Open the file with Open... instead." with the button "Open...". In a second page of the same context (`const second = await page.context().newPage()`) opened with `openApp(second, { fileSystemAccess: false })`, dropping `step-block.ptsm` shows `notification[data-code=assets_unsupported]` reading "The imported geometry of step-block.ptsm is missing, because this browser does not let Partsmith reach files next to the one you chose. Open it in Chrome or Edge, or in the Partsmith desktop app." with no button but "Dismiss", and dropping `cube.ptsm` there shows neither.

Browser engines (6.8):

156. E-156. The projects (`smoke.spec.ts`, tagged `@engines`, so every project runs it; it does not call `openApp`). `test.info().config.projects` names `chromium`, `firefox` and `webkit`, in that order. Each project's `use` has the viewport 1280 by 720 and `deviceScaleFactor` 1. `firefox` and `webkit` have the `grep` `/@engines/` and no `launchOptions.executablePath`; the `grep` of `chromium` is Playwright's default, which matches every title, and its `launchOptions.executablePath` is `PLAYWRIGHT_CHROMIUM_EXECUTABLE` when that variable is set and absent when it is not. `firefox`'s `use.headless` is `false`; `chromium` and `webkit` leave `headless` to Playwright's default (absent from `use`). The `browserName` fixture equals the name of the project that runs the test. Which projects run without `--project` is E-157's to check, since a project's `default` is not part of the configuration a test sees (`FullProject`).
157. E-157. The subset and the default project (`smoke.spec.ts`, not tagged, so it runs in `chromium` alone; it does not call `openApp`). The test runs the command `playwright test --list --reporter=json` of `@playwright/test` in `ui/`, with its own environment, four times: with `--project firefox`, with `--project webkit`, with `--project chromium --project firefox --project webkit` and with no `--project`. A listing loads the configuration and the test files and starts no browser and no web server, so it needs only the web core in `ui/public/core`. The `firefox` listing and the `webkit` listing each hold exactly the engine subset of 6.8, each test once and in that project: the startup check of `smoke.spec.ts` ("the production build starts the WebAssembly core in a cross-origin isolated page") and, for each of the 23 E-test ids of 6.8, the one test whose title starts with that id and a space. The listing with no `--project` holds only project `chromium`, so `pnpm e2e` runs Chromium alone, and its `chromium` tests are exactly those of the listing of all three projects, so no test runs in another engine without running in Chromium.

Print suggestions (3.17, print.md 3.12, ADR 0040), in `print.spec.ts`:

158. E-158. A split suggestion. Drop `oversize.ptsm`, choose the radio "Print", wait for `health-badge` `data-stale="false"` and click "Print check": the `health-item[data-check=bed_fit][data-severity=error]` holds a `health-suggestion` reading "Split at x = 0 mm and y = 0 mm", and `viewport` has `data-split-planes="0"`. Middle-drag from `(cx, cy)` to `(cx + 0.3 H, cy)`: `data-camera-target` is no longer "0,0,5", so the next step can show the view moving. Click that row: `finding-marker` is visible, and `viewport` has `data-split-planes="2"` and reaches `data-camera-target="0,0,5"`. The box and the cube rule both center (0, 0, 5) here; U-236 is what tells them apart. Click "Close print check": `viewport` has `data-split-planes="0"` and there is no `finding-marker`.
159. E-159. A drain hole suggestion. Drop `hollow-resin.ptsm`, choose the radio "Print", wait for `health-badge` `data-stale="false"` and click "Print check": the `health-item[data-check=trapped_volumes][data-severity=warning]` holds a `health-suggestion` whose text matches `^Add a drain hole near \((-8|8), (-8|8), 2 mm\)$`. Click that row: `finding-marker` is visible, `viewport` reaches a `data-camera-target` made of the text's three numbers joined by commas (for example "-8,8,2"), and `data-split-planes="0"`.

### 6.6 Visual tests

Tagged `@visual` in `ui/e2e/visual.spec.ts` and skipped unless `PARTSMITH_VISUAL=1`, because a baseline only matches screenshots from the machine and browser build that took it: they run on the fixed hardware runner (delivery.md 1.2), not in the `e2e` job, whose Chromium is the one `@playwright/test` installs. They use `expect(page).toHaveScreenshot` of the whole window with `animations: 'disabled'`, `maxDiffPixelRatio: 0.002` and `threshold: 0.05`. The per-pixel color threshold is stricter than Playwright's default 0.2, under which a missing 12 percent region fill, a 30 percent problem halo or a recolored 1-pixel edge compares equal, so the baselines could not catch the defects they exist for; screenshots of one build on one machine still differ by at most a few pixels of anti-aliasing, well inside it. Themes are chosen with `page.emulateMedia({ colorScheme })` before `openApp` (settings stay `system`). Before each screenshot the test awaits `document.fonts.ready`, then `document.fonts.load('14px Geist')` and `document.fonts.load('14px "JetBrains Mono"')`, and fails unless each load resolves with at least one face and `document.fonts.check` is then true for both, so baselines never capture fallback fonts (the fonts are served with the app, 2.16.7, so the runner needs no network for them): `check` alone is true for a family without any face (a stylesheet that never loaded) and false for a family no element has used yet (JetBrains Mono on a screen without code, which is every screen but V-21). The rebuild time in the status bar changes from run to run, so the screenshot applies `ui/e2e/visual.css`, which hides `status-recompute` (`visibility: hidden`; the items after it are aligned right, so nothing moves). Baselines live in `ui/e2e/visual.spec.ts-snapshots/` (Playwright's default folder; their names end in `-chromium-linux`, so they apply to the Linux runner only) and are generated with `--update-snapshots=all`, because the default mode rewrites only baselines that fail the comparison. They are reviewed and approved by a human at the gate (technical design 16 and 17); a changed baseline needs reviewer approval.

1. V-1. Empty app, light theme. V-2. Empty app, dark theme.
2. V-3. `cube.ptsm` with "Home view", light. V-4. The same, dark.
3. V-5. `rect-sketch.ptsm` in sketch mode (fully constrained colors, dimensions), light. V-6. `underconstrained.ptsm` in sketch mode, dark.
4. V-7. `overhang.ptsm` in Print with the overhang map and legend, light, seen from "Front right bottom corner view", since the overhang is the underside of the cap.
5. V-8. `cube.ptsm` with the Extrude inspector open in edit mode, light. V-9. `cube.ptsm` with the front face selected and the top edge hovered, light.
6. V-10. `fillet-error.ptsm` with the Problems panel open and the inspector of "Fillet 1" showing its error banner, light. V-11. The same, dark.
7. V-12. `rect-sketch.ptsm` in Model context with the Extrude inspector open and its empty Profile field active (the region fills of 3.12 pass 7 shown), light.
8. V-13. `cube.ptsm` with the Parameters panel open and "Extrude 1" being edited (both side islands docked, the inspector card, the view cube and the viewport controls in the free area's top corners), light. V-14. The same, dark.
9. V-15. Cold start, "Sketch" clicked in `empty-state` (plane picking, iso view): the origin axes in the X, Y and Z colors with their letter labels and the three origin planes in their normals' colors, light. V-16. The same, dark.
10. V-17. `revolve-sketch.ptsm` in sketch mode (a sketch on the Front plane (XZ): its horizontal axis in the X color and its vertical axis in the Z color, around a fully constrained rectangle), dark.
11. V-18. `cube.ptsm` with the body row "Cube" selected (the selection inspector's Size and Center readouts with their axis letters), light. V-19. `cube.ptsm` with "Move body" run from the palette (the X, Y and Z letters inside the Move fields), dark.
12. V-20. `cube.ptsm` with "Home view" at UI scale 2 on 1280 by 720 (the browser closed, the islands, view cube, viewport controls and top-center slot at 200 percent), light.
13. V-21. `parametric.ptsm` with the Parameters panel open and "Extrude 1" being edited (the parameter's name and expression in the code face beside its value in tabular figures, the browser's parameter row in the code face, and the Distance field holding `size` in the interface face, 2.16.7), light.
14. V-22. `cube.ptsm` with the Parameters panel open and "Extrude 1" being edited, as V-13, in forced colors (`page.emulateMedia({ forcedColors: 'active', colorScheme: 'light' })`): every island and card drawn by its 1-pixel system edge, with no shadow, and the checked segments by their outline (3.24).
15. V-23. `cube.ptsm` with "Home view" and the Document menu open below the top bar, over the browser (a popup on `surfaceRaised`: white over the white browser and set apart by its `shadow-lg` in the light theme, a step lighter than the browser in the dark theme, with its separators, 3.2), light. V-24. The same, dark.
16. V-25. `cube.ptsm` at UI scale 2 on 1280 by 720 with "Extrude 1" being edited and the browser opened over the inspector (an overlay island on `surfaceRaised`, a step lighter than the inspector card under it, 3.2), dark.

No baseline is approved yet: all twenty-five need a human reviewer's approval at the next gate, and V-1 to V-12 are taken again with the islands and axis colors of ADR 0030 (`--update-snapshots=all`), since every one of them changes. They were taken in the development container with its preinstalled Chromium 141 (`/opt/pw-browsers/chromium`), not with the Chromium that `@playwright/test` pins, so the fixed hardware runner takes them again before they gate anything. The fixes after the review of the islands (2026-10-15) changed all twenty again, and they were taken again with `--update-snapshots=all`; like the rest, these new baselines need a human reviewer's approval at the next gate. What each shows that earlier drafts of this spec did not, for that review:

- Every baseline: the renamed chrome (3.3, 3.17), and since ADR 0030 the canvas under rounded frame islands with their gutter, outlines and shadows (3.2), the `stage` color in the gutter behind them, the view cube at `top-11 right-9` with its edges in the X, Y and Z colors, the viewport controls in the free area's top-left corner (3.14), and the shapes of 3.23 (`rounded-md` controls, `rounded-lg` menus and chips, `rounded-xl` cards).
- V-1, V-2, V-15, V-16: the origin axes in the X, Y and Z colors with colored letter labels (3.12); V-15, V-16 also the origin planes in their normals' colors.
- V-5, V-6, V-17: the sketch's own axes in the colors of the world axes they lie on (3.16.2).
- V-7: the overhang legend at the bottom of the free area's left column, under the viewport controls (3.17).
- V-8, V-10, V-11, V-12, V-13, V-14: the inspector card placed in the free area right of the viewport controls (3.7); V-10, V-11, V-13, V-14 also a docked side island.
- V-18, V-19: the axis letters of coordinate readouts and fields (3.7, 3.8).
- V-20: the 200 percent layout (3.2, 3.24).
- V-1, V-2: the empty state (3.2), the origin axes of an empty document (3.10) and the ground grid (3.12).
- V-3, V-4, V-9: the ground grid, drawn before the edges (3.12).
- V-5: the grid on the sketch plane (3.12).
- V-6: the profile problem halo under the curve (3.16.2).
- V-7: the Print chrome (3.17) and the view from "Front right bottom corner view".
- V-8: segmented choices that span their field or become a select (3.7), over the grid drawn before the edges (3.12).
- V-10, V-11: the problems button's error icon (3.9).
- V-12: region fills while a field takes regions (3.12 pass 7) and segmented choices that span their field or become a select (3.7).
- Since the review of the islands: the view cube's Home view button outside the cube's top-left corner (every baseline, 3.14); face selection and hover tints mixed in sRGB, orange rather than rose (V-9, V-18, 3.12); sketch axis labels beside their axes (V-5, V-6, V-17, 3.16.2), and the sketch origin and the dimension to the Z axis framed with the sketch that refers to them (V-17, 3.16.1); the inspector's footer under a `divider` rule outside its scrolling body, and the select's own chevron (V-8, V-10 to V-14, V-19, 3.7); side panel headers over a `divider` rule (V-10, V-11, V-13, V-14, 3.2); no separator in the empty Sketch toolbar while a plane is picked (V-15, V-16, 3.3); gestures of the empty state kept on one line (V-1, V-2, 3.2); and at 200 percent Search and Export as icons beside the whole document name (V-20, 3.2).
- Since ADR 0031 (2026-10-15), all of V-1 to V-20 change again and are taken again with `--update-snapshots=all`, and V-21 is new: Geist with its serifed capital I in place of Inter on every screen, and Geist's tabular figures in place of JetBrains Mono for every number, in the status bar, the readouts and measurements (V-18), the fields (V-8, V-10 to V-14, V-19), the dimension labels (V-5, V-6, V-17) and the overhang legend (V-7). JetBrains Mono now shows in V-21 only.
- Since the review of the fonts (2026-10-16): the Problems panel's first row clear of its header rule (V-10, V-11) and the Parameters panel's transaction line clear of its header rule (V-13, V-14, V-21), both 3.23. All twenty-one were taken again with `--update-snapshots=all` after these fixes; a second run on 2026-10-16 matched every one (it differed only in V-15, V-16, V-19 and V-21, by at most 2 color levels in at most 27 anti-aliased pixels, well inside the comparison, so those baselines were kept). These baselines, V-21 among them, need a human reviewer's approval at the next gate before they gate anything.
- Since ADR 0032 (2026-10-16), all of V-1 to V-21 change again and are taken again with `--update-snapshots=all`, and V-22 to V-25 are new: no outline on any island, card, popup or chip (the outlines the bullets above name are gone); white islands over a flat `zinc-100` viewport in the light theme and `zinc-900` islands over a flat `zinc-950` one in the dark theme, with no viewport gradient and the gutter in the viewport's tone; the dark shadow at 20 percent; menus, popovers, tooltips and notifications on `surfaceRaised`, a step lighter than the islands in the dark theme (V-24, V-25), and translucent dark hover and tonal fills; tooltips and the timeline's drop hint at `shadow-lg`; the view cube's Home view button with a `shadow-md` (every baseline); segmented controls (the context switcher in every baseline, the inspector's choices in V-8, V-10 to V-14 and V-19) as a tonal track with a raised thumb in place of the orange checked segment; tonal default buttons (the empty state's in V-1 and V-2, the plane buttons in V-15 and V-16, the inspector's footer, and the error banner's fix buttons on their `banner` fill in V-10 and V-11) and Search and Export as ghost buttons; no rule under side panel headers (V-10, V-11, V-13, V-14, V-21), above the inspector's footer (V-8, V-10 to V-14, V-19) or between parameter rows (V-13, V-14, V-21), no line in toolbar separators, and no rule inside the sketch header (V-5, V-6, V-17); and the dark grid in `zinc-700` (the dark baselines V-2, V-4, V-6, V-11, V-14, V-16, V-17 and V-19). These need a human reviewer's approval at the next gate (open issue 13).
- Since the reviews of the flat islands (2026-10-16), all twenty-five are taken again with `--update-snapshots=all`: the browser on `surfaceRaised` while it floats over the free area (V-25, a step lighter than the inspector under it), the top bar's Toggle browser and Dark theme pressed in the neutral `hover` fill rather than orange (every baseline with the browser open, and the theme toggle in every dark one) and Search and Export without a border in forced colors (V-22). The web core of 2026-10-13 reads format version 1 only, while the fixtures are version 2 (ADR 0025) and differ from version 1 by that number alone, so these baselines were taken with the fixtures read as version 1 in memory, outside the repository. Like every baseline above, they need a human reviewer's approval at the next gate.
- The owner reviewed V-3, V-4, V-13, V-14 and V-24 of the flat islands on 2026-10-16 and approved the direction ("overall it looks good so far"). That settles the look of ADR 0032; the baselines themselves still need approval at the gate, after the fixed hardware runner takes them again.
- Since the wasm step rebuilt the web core (2026-10-16, open issue 17), V-19 changes once more and fails until it is taken again: the Move X, Y and Z fields start at `0`, the catalog default of `move_body`'s `translation`, where the local core of 2026-10-13 that took every baseline so far gave no default and left them empty (3.7). The primitives' `position` defaults of the same fix show in no baseline.

### 6.7 Performance tests

Tagged `@perf`, skipped unless `PARTSMITH_PERF=1`, run on the fixed hardware runner (technical design 16; a regression over 10 percent blocks the merge). Until that runner exists (delivery.md 1.2) they run by hand on the reference machine before a release, with `PARTSMITH_PERF=1 pnpm -C ui e2e perf.spec.ts`. Web budgets from section 5, and protocol.md 5 for P-12; the desktop budgets run in the desktop harness when the host lands. The initial JavaScript budget of section 5 has no P-test: it depends only on the build, so `pnpm check:dist` checks it on every CI run, as delivery.md 2.6 defines it (delivery.md tests 26 to 33).

The tests live in `ui/e2e/perf.spec.ts`. Each title starts with its id and ends with `@perf`, and each test starts with `openApp` and opens the fixture it names with `dropFile`, unless it says otherwise. A test reads only the entries of its own interaction: it takes `pageTime` right before the interaction and passes it as `since` to `measures` and `longTasks` (6.3). "Median" and "95th percentile" are `percentile` with 0.5 and 0.95. "No long task over 50 ms" means `longTasks` holds no duration over 50 ms for the interaction, with `watchLongTasks` called before `openApp`. Before asserting a budget, a test adds one test annotation of type `perf` per measured value, written `{id} {statistic}: {value} ms (budget {budget} ms)` with the value rounded to 0.01 ms (for example `P-4 ps:rebuild: 1840.25 ms (budget 3000 ms)`, or `P-2 longest long task: 0 ms (budget 50 ms)` when there was none), so every run reports its numbers whether it passes or fails; a statistic with no entry to compute it from is reported as `NaN` and fails its budget, after the test's other values are reported. A test that needs longer than Playwright's default timeout (opening `dense.ptsm` takes about 25 seconds) sets its own.

1. P-1. Load the app once, reload: `ps:usable` is at most 4,000 ms after `performance.timeOrigin`.
2. P-2. `sketch-500.ptsm`: drag `e-drag.end` along a 2 s path with 120 pointer moves: 95th percentile of `ps:sketch-drag` at most 16 ms; no long task over 50 ms.
3. P-3. `part-50.ptsm`: edit the first extrude and type 20 distances, one every 300 ms: median `ps:preview` at most 250 ms; no long task over 50 ms.
4. P-4. `part-50.ptsm`: change parameter `base`: `ps:rebuild` at most 3,000 ms.
5. P-5. `part-50.ptsm` opened, New, opened again: the second `ps:open` at most 1,000 ms.
6. P-6. `dense.ptsm`, home view: a 3 s Shift+middle-drag orbit along a path of 180 pointer moves: 95th percentile of `ps:frame` at most 16.7 ms; no long task over 50 ms.
7. P-7. `dense.ptsm`: 100 hovers with a still camera: 95th percentile of `ps:pick` at most 1 ms.
8. P-8. `palette-2000.ptsm`: open the palette and type 10 characters: 95th percentile of `ps:palette` at most 8 ms.
9. P-9. `part-300.ptsm` (300 features whose fillets and face sketches use the extrudes before them): drag the last feature along the whole timeline with 120 pointer moves: 95th percentile of `ps:drop-check` at most 1 ms.
10. P-10. `part-50.ptsm`: in the Parameters panel, set the expression of `base` 20 times, alternating `250 mm` and `240 mm`, each with Enter once the previous change's `ps:rebuild` is recorded: 20 `ps:apply-events` measures (one per change, each applying the recipe and statuses of 50 features), 95th percentile at most 8 ms.
11. P-11. `part-50.ptsm`: in the Parameters panel, select the whole expression of `base` and type the digit 2 twenty times, each keystroke once the previous text's `ps:expression` is recorded: 20 `ps:expression` measures, median at most 50 ms.
12. P-12. Worker transfer (protocol.md 5). A test page in `ui/e2e/pages/`, served by a Vite dev server as `viewport.spec.ts` serves its page, starts a module worker. 20 times, the page asks the worker for a frame and waits for it, so one frame is in flight at a time. The worker encodes with `encodeFrame` a new response frame (one `undo` response with id 1 to 20 in turn, and one `u8` buffer of 50,000,000 bytes), reads `performance.timeOrigin + performance.now()`, posts `{type: 'frame', frame}` (protocol.md 2.11) with `[frame]` as the transfer list, and then reports the time it read and the state of `frame` in a second message. A transfer's time runs from the worker's reading to `performance.timeOrigin + performance.now()` read first in the page's `message` handler for the frame (HR-Time makes the two comparable across contexts): median at most 2 ms. After every `postMessage` the worker's `frame` is detached (`detached` true, `byteLength` 0), and every frame the page receives decodes with `decodeFrame` to its response and a `Uint8Array` of 50,000,000 bytes.

The engine's web-column benchmark (engine.md section 5, tests 155 to 157) shares this harness from a file of its own, `ui/e2e/engine-budgets.spec.ts`: its rows are tagged `@perf`, run with `PARTSMITH_PERF=1 pnpm -C ui e2e engine-budgets.spec.ts`, and write their `perf` annotations as these tests do. Rows 2 to 17 go through `expectWithinBudgets` (`expectWithinBudgets(id, measured)`, which adds `{id} {statistic}: {value} ms (budget {budget} ms)` for each measured value and then holds each to at most its budget), moved from `perf.spec.ts` to `ui/e2e/support/perf.ts`, which both files import, with the statistic `median`; row 1 adds its bytes annotation and checks that its bytes are below 4294967296 itself. Run one of the two files at a time: each file runs its tests one after another in one worker (`mode: 'default'`), but Playwright runs two files in two workers at once, so the two side by side slow each other down. A local run reports through the `list` reporter, which prints no annotations, so a run that records its numbers adds the JSON reporter, as engine.md section 5 gives it for its file: `PARTSMITH_PERF=1 PLAYWRIGHT_JSON_OUTPUT_FILE=playwright-report/perf.json pnpm -C ui e2e perf.spec.ts --reporter=list,json` writes the annotations of every test to `ui/playwright-report/perf.json`.

### 6.8 Browser engines

`ui/playwright.config.ts` always defines three projects, in this order: `chromium` (`devices['Desktop Chrome']`, with `launchOptions.executablePath` from `PLAYWRIGHT_CHROMIUM_EXECUTABLE` when that variable is set, so a local run can use a preinstalled Chromium), `firefox` (`devices['Desktop Firefox']` with `headless: false`, below) and `webkit` (`devices['Desktop Safari']` with `deviceScaleFactor: 1`, so every project has the positions of 6.5). `firefox` and `webkit` have `grep: /@engines/` and `default: false`, the project option of Playwright 1.64 under which a project runs only when `--project` names it. So `pnpm e2e` runs `chromium` alone and needs only Chromium installed, as CI's `e2e` job and a local run have it, and the platforms workflow runs one engine per job (`pnpm e2e --project firefox`, delivery.md 2.14); E-156 checks the projects and E-157 which tests each runs. The executable path belongs to `chromium` alone, since another engine cannot start Chromium's executable. The `@perf` and `@visual` exclusions of the configuration apply to every project. `chromium` and `webkit` run headless, as CI's `e2e` does. `firefox` runs headed (`use.headless: false`): on the runner, headless Firefox gave the canvas no WebGL2 context, so the app showed its unsupported page (3.1) and every subset test but E-2 and E-156 failed (open issue 41). Headed, it runs under the virtual display the platforms workflow starts with `xvfb-run` (delivery.md 2.14); on a machine without a display a local run needs `xvfb-run` too. That is neither a defect of the app nor of a test, and no test leaves the subset for it; another engine that needs the same gets the same option, recorded in open issue 41.

**The engine subset** is the tests tagged `@engines` with Playwright's `tag` option (`test(title, { tag: '@engines' }, body)`, which `grep` matches as it matches a tag written in a title, so the titles stay as they are). Each runs as written in all three projects. They are 24 tests: the startup check of `smoke.spec.ts` (the production build starts the core in a cross-origin isolated page, 6.3), E-1 to E-6, E-8, E-9, E-10, E-12, E-13, E-29, E-33, E-41, E-47, E-52, E-53, E-56, E-127, E-128, E-144, E-147 and E-156. Together they reach each feature of 1.1 that an engine must have: the core (WebAssembly with exceptions in a module worker, cross-origin isolation), WebGL2 drawing and its ID-buffer picking, pointer input on the canvas, the Popover API (menus, the palette, popovers), sketch mode's chunk loaded on demand, the file chooser, drag and drop and downloads where the File System Access API is missing, IndexedDB and Web Locks (recent files, recovery copies, two tabs), `beforeunload`, `prefers-color-scheme` with a stored setting, and fonts from the app's own origin.

A test can join the subset only when it runs as written in every engine: it reaches no File System Access picker (it opens the app with `fileSystemAccess: false`, or its files only drop and download), and it uses nothing that Playwright or the app offers in Chromium alone: `useFolders` (its pickers stand in for an API only Chromium has, over an origin private file system whose permission answers only Chromium gives), `stallCore` (`page.route` serving the core worker's requests), `watchLongTasks` (`longtask` entries), the `forcedColors` emulation, clipboard permissions, and the `@visual` and `@perf` tests, whose baselines and budgets are Chromium's. Every other E-test runs in `chromium` alone, among them E-150 to E-154, which test the File System Access API itself (open issue 39).

A subset test that fails in one engine is a defect of the app in that engine, or of the test where it relies on something only Chromium does. The app is fixed, or the test without weakening what it checks in Chromium, and until then the failure is recorded in open issue 41. Leaving a test out of the subset is a change to this section that names the engine feature it lacks, with an open issue, never `test.skip`, `test.fixme` or a condition on the engine in the test (delivery.md 2.14).

**What runs where.** The development containers of this repository have Chromium alone, so a local check of the subset is `pnpm e2e --project chromium --grep @engines`, which runs the subset, E-156 among it, in Chromium, and `pnpm e2e --grep E-157`, whose listings show what Firefox and WebKit would run without launching either. The runs in Firefox and WebKit are CI's alone (delivery.md test 64).

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
10. **Google Fonts dependency.** Resolved by ADR 0031 (the owner's decision of 2026-10-15). `src/index.css` used to load Inter and JetBrains Mono from Google Fonts (ADR 0013), so offline use fell back to system fonts, the visual tests needed the network, every launch sent the user's IP address to Google, the CSS `@import` added a request chain before the first paint, and the desktop policy allowed two remote hosts. The fonts are now files of the app (`public/fonts`, 2.16.7), with no npm package added: Geist preloaded from `index.html` and JetBrains Mono fetched on the first code text, both from the app's origin. Their license, the SIL Open Font License 1.1, joins the license policy's review at the gate (ADR 0031) and the third-party notices (m1-gap-list B-2).
11. **Wide-gamut colors.** Many Tailwind 4 palette steps lie outside sRGB (2.16.5), the accent's `orange-500` and `orange-600` among them. The DOM renders their OKLCH values, so on Display P3 screens the interface's orange and red are more saturated than the same roles in the viewport, which draws the sRGB-clipped hex, and than stored body colors. Matching them needs a P3 WebGL drawing buffer (`drawingBufferColorSpace = 'display-p3'`) with P3 values in `roles.ts`, or sRGB arbitrary colors in the DOM, which the Tailwind guideline discourages.
12. **Default display color in io.md.** Resolved: io.md now uses the `body` role value `#9f9fa9`.
13. **Design-system values need human review.** The mapping of 2.16.3 to 2.16.5 is a design decision for the gate: the zinc steps of surfaces and text, `orange-600` and `orange-500` as the accent (light marks need `orange-600` to reach 3:1 on white), the yellow hazard-sign treatment of warnings, `red-700` and `red-500` for errors, the cool analysis ramp in place of a warm one, the default body color and the swatches. An orange body hides the orange hover and selection tints of its faces; its edges and the selection inspector still show the selection. ADR 0030 adds to the review: the island tiers of 3.2 (frame islands with `shadow-md`, cards, chips and popups, the radii of 3.23, a 0.5rem gutter), and the axis roles `axisX` `rose-600`/`rose-400`, `axisY` `emerald-700`/`lime-400` and `axisZ` `sky-700`/`sky-300` (2.16.3 to 2.16.6), whose light X and Y come closest under protanopia (0.079) and whose dark Y comes within 0.019 of the warning yellow under deuteranopia, carried by the letters, the thicker hovered and selected strokes and the hazard shape of warnings. ADR 0032 (the owner's flat islands of 2026-10-16) replaces ADR 0030's outlines and surfaces and adds: no outline on any island, card, popup or chip, with `forcedEdge` (a transparent 1-pixel border that only forced colors draw, 3.24) in place of the `border` role, and state outlines drawn the same way on selected, pressed and active elements (3.23); `surface` (`white`/`zinc-900`) for every island, card and chip over one flat `viewport` (`zinc-100`/`zinc-950`, `stage` the same), 1.09:1 and 1.12:1 at every point, the light step being the most the accent's 3:1 marks allow, so the light islands' top edges show by the step alone (ADR 0032 weighs the alternative, a `zinc-200` light viewport at 1.26:1 with `highlight` moved to `orange-700`); `surfaceRaised` (`white`/`zinc-800`) for menus, popovers, tooltips, the suggestions list, the palette, notifications and the drop overlay, a step over a dark island and set apart by the stronger shadow alone over a white one; translucent dark fills (`hover` and its `active` facet `zinc-700` at 60 percent, `tonal` at 40 percent, `divider` white at 10 percent), an opaque `hover` facet for chips over the viewport, and `banner` facets for buttons on status banners (`zinc-950` at 5 and 10 percent light, white at 10 and 15 percent dark); `elevation` black at 10 and 20 percent, `shadow-md` for what stays and `shadow-lg` for what comes and goes, shadows that show below and beside an element and not on the dark viewport; tonal default buttons, ghost Search and Export, and segmented controls as a `tonal` track with a raised `thumb` (`white`/`zinc-700`, `shadow-xs`) in place of the `selection` fill; the dividers that stay (menu separators, the palette's search field, and the edges of scrolling bodies while content runs under a header or footer) and the rules dropped (3.2); the viewport's gradient dropped; the dark grid moved to `zinc-700`; and the measured rows of 2.16.6, among them the light default body at 2.38:1 against the viewport, as on the old gradient's bottom, and the curved outlines of the White and Black swatches at 1.36:1 and 2.20:1 at their lowest. Surfaces of one step still meet with only a shadow between them beside a submenu and, under 65rem, where a popup opened from an overlay island, or a notification, lies over it: the browser and the side panel float there on `surfaceRaised`, a step over the free area's cards (V-25). The top bar's Toggle browser and Dark theme show their pressed state in `hover`'s neutral `active` fill rather than `selection`, which stays for tools, modes and the viewport toggles (3.23). A pressed toggle `Button` (the dimension editor's Reference) measures 1.04:1 light and 1.06:1 dark against its unpressed `tonal` fill, so its state reads by hue, like the other pressed toggles against their islands; the ring candidate below would serve it too. The visual baselines V-1 to V-25 are the review material. The design review of the islands raised two more values for the gate, left unchanged here because they are palette decisions: the dark `selection` step `orange-950` (`#441306`) lies 0.022 (OKLab) from the dark `errorSubtle` (`red-950`) and 0.034 from the dark `warningSubtle` (`yellow-950`), so a pressed toggle and a selected row read maroon rather than orange next to an error or warning banner (a candidate: `zinc-800` with a 2-pixel `orange-500` inset ring, which keeps clear of the focus outline); and text selected in inputs keeps the browser's own highlight, a blue that the viewport uses for under-constrained sketch geometry (a candidate: a `selection:` facet in the accent's light steps, for example `orange-200` and `orange-900`, which needs a contrast row in 2.16.6).
14. **Scope of the runtime style exception.** The guidelines name the floating inspector and dimension labels as runtime positions. This spec applies the same exception, only through `positionElement` and `orientElement`, to constraint glyphs, draw inputs, inference glyphs, axis labels, the timeline's drag ghost, menus, popovers and tooltips anchored to their triggers, the name suggestions of expression fields (2.13) (Tailwind 4.3.3 has no anchor positioning utilities, and the Popover API's top layer ignores ancestor positioning) and the view cube's rotation, and uses SVG attributes for overlay marks and body color swatches. It also uses four kinds of arbitrary values: the ligature property of editable code (`[font-variant-ligatures:no-contextual]`, 2.16.7; Tailwind has no ligature utility, and the font families are theme variables since ADR 0031), the UI scale percentages, the inspector's `max-h-[calc(100%-2rem)]` and the app root's grid templates (3.2; Tailwind has no utility for a grid of auto and fraction tracks). The maintainer should confirm this scope.
15. **One interface for everyone (decision).** Partsmith has no beginner mode, expert mode, simple view, skill setting, first-run flag or onboarding UI that changes which tools exist ([guidelines](../guidelines/README.md), "One interface for everyone"). Newcomers and experienced users see the same screens with the full power of the app, and approachability comes from how those screens present it: plain words, starting values that work, availability reasons before a click, hints in context that cost an experienced user nothing, and typed, keyboard and direct paths in the same surfaces. Disclosure is the same for everyone (More options, hover help, panels opened on request) and keeps power one obvious step away.
    - Added: the empty-document first step, origin axes and grid (3.2, 3.10, 3.12), which follow the document, not the user; right-drag orbit and pan in both presets (3.13); availability reasons for feature commands (2.10); the hand-off from a finished sketch to the next feature (3.3, 3.16.11) and the camera that leaves the sketch plane (3.16.11); holes placed by clicking a face, with a real sketch behind them (3.7); sketch step prompts and visible typed sizes (3.16.3, 3.16.4); starting values that work on printed parts and reuse of the document's last values (2.12); picks and typed digits that always land in the command (3.7); new bodies for copies and split parts (2.12); Reverse beside the distance and cuts that go into the part (2.12, 3.7); parameter edits during a command (3.7); undo inside a command and Restore (3.7); plain sketch status (3.16.8); failure notifications that follow the fix (4.2); descriptions and field help on hover (2.10, 2.12); the palette's other-app names, context ranking and browsable empty list (2.10, 3.4); view-named planes with one-click plane buttons and a Sketch radio that edits the selected sketch (3.3, 3.10, 3.16.1); next steps, a size field and measurements in the selection inspector (3.8); "More options" that shows what it hides (3.7); parameter name completion and one-click creation (2.13, 3.7); export status (3.18); one name per concept and the printer in use (3.17); the viewport context menu and Repeat (3.5, 3.13); faces for fillet and chamfer edges (3.7); Lay on face (3.17); the keyboard and mouse reference (3.5).
    - Left out on purpose: letters routed into values while drawing or during commands (they stay shortcuts, so S, V and the feature letters always work; digits and Tab cover typed entry); committing a command that ends implicitly (it would create features nobody asked for; Restore recovers the work instead); start values remembered in settings across documents (hidden state that breaks parameter names; reuse comes from the document); a separate "Coming from Fusion / FreeCAD" panel (the tagged keywords show the mapping in the palette everyone uses); a plane-pick popover, a dotted underline under every label with help and a hover tooltip on every numeric field (visual noise for little gain; the label tooltip, `cursor-help` and the keyboard and mouse panel carry the same help); clicking a sketch curve during plane picking to edit that sketch (a click near a visible sketch line would open the old sketch instead of starting a new one; the Sketch radio and double-click edit it); distances and angles between two selected items (they need the core's two-target `measure`, 1.2); renaming Boolean to "Combine" (the manifest label belongs to the core; "Combine (Fusion)" is a keyword); a toolbar width ladder that drops tools at fixed widths (tools that do not fit move into the visible More tools menu instead, 3.2); and placeholder rows in empty browser groups.
    - Later, outside this spec: on-canvas handles (1.2; they need a handle anchor per field from the core); dependency marks drawn on every timeline item and reorder drops that refuse to break a dependency (M1 warns while dragging and lists "Used by" from `list_uses`, 3.9); face edges as sketch snap and dimension targets (sketch.md and engine changes); custom printer profiles (`put_printer_profile`, with a decision on documents that name a profile another machine lacks); one file per body and a mesh tolerance in export; a curated sample part opened from the empty state; trackpad two-finger pan and orbit.
    - Companion changes for human review at the next gate: features.md message wording (core messages that pointed to a "repair panel" or said "op"; done with the reference stability study: features.md sections 4.1 and 4.2 follow the error guidelines, whose message lint forbids both), because the UI shows core messages verbatim; moving `FEATURE_DESCRIPTIONS`, `FIELD_HELP`, `START_VALUES` and the body-creating `target` rule into `FeatureManifest` and `FieldHint` for plugins, the CLI and Python (a schema change that needs an ADR), and marking `reverse` essential in the extrude, revolve and hole manifests so the inline Reverse rule of 2.12 needs no override; hole.md's Later entry "Placement on a face by click without a sketch", which the UI now provides with a helper sketch (3.7); and technical design 13's onboarding bullet, whose "Coming from Fusion / FreeCAD" panel is the palette's tagged keywords in M1.
16. **Top-center slot and view cube in narrow viewports.** Resolved by the island layout (3.2, ADR 0030). The slot used to keep only `inset-x-32` from the viewport's sides while the turned cube reaches 8.92rem from the right edge, so a wide chip or sketch header covered the cube's left corner by up to 1rem, and the wider `inset-x-36` left too little room at 200 percent with the browser open. Now the cube sits at `top-11 right-9`, reaching at most 9.81rem from the free area's right edge and 11.01rem below its top (3.14), and the slot measures itself (`@container/slot`): while it is at least 36rem wide its column keeps 10.5rem from both sides of the free area (0.5rem clear of the Home view button since ADR 0032), clear of the cube and of the viewport controls in the other top corner, and in a narrower slot it spans the free area 12rem below its top, under both, rising toward the top when the free area is too short for that. At 200 percent the browser and side panel float over the free area below a 2080 CSS-pixel window (3.2), so the free area, and with it the slot, keeps the window's width. E-137 checks these cases.
17. **Core defects the end-to-end tests found.** Fixed in the core and confirmed on the web core; open only until V-19 is taken again. The core (crates/) is outside the UI's scope, so these tests were marked `test.fixme`, each with a comment that named its defect, until the core was fixed; the marks are gone. E-57 `move_body`, E-93 and E-108 `cube-features.ptsm` pass on the web core the wasm step of the reference stability plan rebuilt on 2026-10-16 (`scripts/build-wasm.sh`, `partsmith_core.wasm` 19,294,003 bytes). The native build gives `cube-features.ptsm` the stated result (all 4 features ok, volume 3323.327412, 15 faces; features/shell.md test 23). The rebuilt core changes the visual baseline V-19 (section 6.6): its Move X, Y and Z fields start at `0`, the default the catalog now gives `move_body`'s `translation`, where the 2026-10-13 core that took the baseline left them empty, so `PARTSMITH_VISUAL=1 pnpm e2e visual.spec.ts` fails V-19, and only V-19, differing in nothing but those three fields' values and `mm` suffixes, until it is taken again on the rebuilt core (`--update-snapshots`), with a human reviewer's approval like every baseline; the issue closes then.
    - Shell too thick after a fillet: `cube-features.ptsm` (a 2 mm fillet on the cube's 4 vertical edges, then a 2 mm shell removing the top face) opened with "Shell 1" failing with `shell_too_thick` ("The thickest wall that works is about 1.9 mm"), and every wall at least as thick as the fillet radius failed, because OCCT's join offset cannot take a convex round to radius zero or below. The kernel now builds such walls with sharp inner corners where the rounds' offsets vanish (kernel.md section 3.4.8 item 6 and tests 131 and 132, features/shell.md tests 19, 20 and 22), so E-108 checks `cube-features.ptsm` again.
    - No schema default for fixed-length arrays: the feature catalog's params schemas of `move_body` `translation` and of the primitives' `position` (box, cylinder, sphere, cone, torus) carried no `default`, since schemars leaves out a default that `skip_serializing_if` would skip, so the inspector started Move X, Y, Z and Position X, Y, Z empty (2.12 rule 3) and setting one of them sent a 1-item array, which the core refused. The schemas now carry `["0", "0", "0"]` (features.md section 3.3 and test 15), so the fields start at 0, and E-57 `move_body` and E-93 run again.
18. **Candidates find their actions by label (needs human review with ADR 0024).** The protocol says each repair candidate "is also an action of the message", but nothing on the wire says which one: a candidate's `Use` action comes among the timeline fixes and removals in the order of features.md section 4.4, not at the candidate's index. Repair mode therefore finds it by its label, `Use {description}`, `Use only {description}`, `Attach to {description}` or `Attach '{sketch}' to {description}` (engine.md section 3.22, `candidateAction`). An `action: u32` on `RepairCandidate` and `RegionCandidate`, the index of the message action that stores it, would make the link explicit at the cost of a schema change.
19. **Quotes in the older UI-owned texts.** The error guidelines name a feature by its label in single quotes, as the core and this spec's new texts of losses, causes and uses do (4.1). The UI-owned texts written before them (`feature_failed`, `features_failed`, `deleted`, `command_canceled`, `profile_problems`, `export_failed_feature` and others) name features without quotes, and E-26, E-81, E-95, U-110 and U-119 check those texts. Quoting them is a small change with a large test diff and needs a decision.
20. **Loss notification texts beyond the study's template (needs review).** The study gives every loss notification the text "{label} lost {repair description}." For `reference_guessed`, `reference_widened`, `region_resized`, `region_remapped` and `sketch_turned_over` nothing is lost (the element is still used, more of it is used, the area changed, a region of the same size and place is used, the drawing is mirrored), and a loss without repair data has no description, so those read `feature_check` ("'{label}' needs a check after this change.") with "Check '{label}'", to stay true (error guidelines, "Never in the message text"). A chain counts only the features newly in error ("so {n} features after it failed too"), and such a notification is an error, not the study's warning, because it takes the place of the failure notification of the same change, which errors keep until dismissed.
21. **The limit line reads `suggestion`.** `Diagnostic` carries no field for the largest value the "too large" search found, so the line under the field (3.7) shows the message's `suggestion`, and none when the bare-number rule makes that suggestion the typed number in mm (features.md section 4.4). A `limit` field on the wire would remove the exception.
22. **Test numbers.** The study numbers its Playwright tests E-105 to E-122, but 6.5 had already used E-105 to E-108 (layout and fixture tests). Its E-109 (R03), E-110 to E-121 (the newcomer error corpus, whose index names those numbers) and E-122 keep their numbers here; its E-105 to E-108 (R16, R41, R28, R06 (a)) are E-123 to E-126. Its U-127 to U-143 keep theirs, and U-144 to U-148 cover what this spec added to make them exact.
23. **The dimension editor's wording for a value the sketch cannot take.** Newcomer corpus NC08 says the editor offers "Add as reference", and the study's sketch messages add "Delete {title}". The editor keeps "Add reference", the label E-18 checks for a new dimension, so one action has one name, and offers no Delete, because a candidate solve carries no actions (engine.md section 3.6) and a constraint's title reaches the UI only inside a message. The corpus now follows this spec (NC08 says "Add reference" and notes that E-117 opens a document already holding 80 mm, open issue 24); a Delete in the editor would need the probe to carry the conflict's fixes (a protocol change).
24. **NC07 and NC08 cannot be typed in the UI.** The dimension editor refuses a dimension that conflicts and a value the sketch cannot take before anything is stored (3.16.7), as "Prevent first" asks, so E-116 and E-117 open documents that already hold the problem, and NC08's cause line, which needs the edit to be stored in the session, is checked by engine.md test 150 only.
25. **Re-attaching a sketch (needs review).** The study's repair mode replaces a reference through the active field of the inspector, but a sketch's plane has no inspector field: editing a sketch enters sketch mode. 3.16.1 re-attaches a sketch through the plane picking of a new sketch, with the lost face's ghost and the candidates in the header, as one undo entry labeled "Attach '{label}'". The pick reads the model at the end of the timeline, not the state just before the sketch, so what a feature after the sketch made is refused (`attach_after`) instead of being stored as a plane the sketch can never find; focusing the sketch while picking, as editing a feature does, would read every face in that state, which engine.md open issue 15 assumes of repair mode, and would make a placement in world coordinates exact also when a Move body after the sketch tilts the face, at the cost of a transaction that every way out of plane picking must cancel. A pick keeps the motion of a sketch without motion (3.16.1); for a sketch whose motion the UI cannot see (on a face with a fallback in body coordinates, or on a datum plane) it stores a face as a new sketch does, and a picked datum plane brings the datum's motion, so such a pick can still move later features with no message, as engine.md open issue 16 describes for the core's `Attach`, whose decision covers both.
26. **Fixes inside a command.** `apply_fix` edits the records, while the inspector keeps the command's params as its own copy. 3.7 therefore re-reads the record after a fix, makes the fix one history step, treats the fix's edits of other features like parameter edits, and disables `apply` actions while a preview is pending, so a fix offered for an older preview is never sent. This replaces the UI's own "Use {suggestion}", whose value the core's `Use` action now carries, so a fix has one path everywhere.
27. **Warning border and bar colors (design review at the gate).** U11 and U7 ask for a field border and an insertion bar in `warningMark`, whose `yellow-400` fill reaches only about 1.6:1 on white. Its new border and background facets use `warningText`'s steps (`yellow-800` and `yellow-300`, 6.22 to 14.99, 2.16.6), which keeps warnings yellow-brown and never orange. The values join open issue 13's review.
28. **Sketch commits no longer write the fallback.** `commitSketchChange` used to store `solution.placement` as a face sketch's fallback on every commit (3.16.3). That placement is in world coordinates, a fallback with `fallback_frame: body` is not, and features/sketch.md lets only an attach, a committed drag and the core's fixes write it, so a document opened, rebuilt and saved without edits stays unchanged. Commits now keep the stored `plane`; the sketch's placement on a lost face comes from the run itself (features/sketch.md section 3.3).
29. **Faces picked as edges ask for each edge's full reference.** Clicking a face in an Edges field (3.7) now calls `pickReference` once per edge, so a fillet picked that way stores anchors like a click on the edge does (engine.md section 3.9), at the cost of one request per edge (four for a box face). A `pick_reference` that takes several picks would make it one request.
30. **Stopping a core that is still working.** A call that runs for minutes or never ends (an endless loop in the kernel) used to leave "Rebuilding" with no way out but reloading the page. After 10 s without an answer the UI offers "Stop and restart core" (4.4), which on the web terminates the worker: the changes after the settled recipe are dropped with a notice, and the undo history and the core's caches are lost, as with any restart. 10 s is well above the web budgets of section 5 (up to 3 s for a full rebuild of 50 features), so a model that is only slow seldom sees the panel, and nothing stops unless the user asks. Only requests the user waits on count: offering a stop for a background request, such as a print check the user did not ask for, would trade the undo history for work nobody is waiting on, and the check's own badge already shows it is busy. Request cancellation (protocol.md section 1, Later) would stop only the running call and keep the history; the desktop needs it before it can offer the button (desktop.md open issue 9).
31. **The canvas under the islands.** The canvas fills the window (3.2), so at 1280 by 720 with the browser open it draws 921,600 CSS pixels where the docked canvas drew 597,408 (1016 by 588), about 1.5 times as many fragments, most of the extra under opaque islands. The frame budget of P-6 (section 5) still applies. If it fails, the backend can skip what no one sees: a stencil or scissor pass that leaves out the islands' rectangles (their rounded corners and the gaps between them stay drawn) would bring the fragment count back near the free area's, at the cost of passing the islands' rectangles to the backend.
32. **Framing clear of the chrome inside the free area (needs a decision).** Fit, Home, the standard views and `enterSketch` frame into the free area, or into its part beside an open inspector (2.11), but not clear of the top-center slot's content or the view cube, which also lie in it. At 200 percent UI scale on a 1280 by 800 window the empty-state card covers the origin of an empty document, and the plane-picking header and hint cover the labels of the origin planes; at 100 percent a fitted sketch's top-right corner can lie under the cube. A top inset at the bottom of the slot's column (and a right inset for the cube when the fit is limited by width) would avoid this, but the slot's content is drawn by React after the store change that frames (a sketch session, a new document), so the controller cannot measure it at that moment; at 200 percent the empty-state card covers more than half of the free area, where the inspector's rule would fall back to the whole area; and the e2e helpers of 6.3 (`toScreen`, `sketchToScreen`) assume framing in the whole free area. Likewise, when neither side of its items has room for the inspector, it opens at the free area's top-left corner and may cover part of them (3.7); opening it on the roomier side and framing the items once beside it would contradict "opening, moving or closing it moves no camera" (2.11, ADR 0030). Both need the owner's decision.
33. **Geist on Windows (human check at the gate).** The variable Geist file has no glyph instructions (2.16.7), and Windows renders unhinted outlines at small sizes softer than macOS and Linux do. A reviewer looks at the app on Windows at 100 percent display scaling and UI scale 1, where `text-xs` (the status bar, captions and the Parameters panel) is 12 CSS pixels and `text-sm` 14, and at the font study's 12, 13 and 14 pixel rows, and records the result per size. If it looks soft, the fallback is the `geist` package's own hinted static files, unmodified: `Geist-Regular.woff2` (45,244 bytes, SHA-256 `d8bce822db092746889bcf3f57350b41f53708b025458fe7af30729ec4ce0df2`), `Geist-Medium.woff2` (46,372 bytes, `b0a0867cda44efef4529a4b13ce37fd9fd6e1597708615287542a51bc7452ab4`) and `Geist-SemiBold.woff2` (46,596 bytes, `b1e6a1dd2122485d0a1f3a8d30a45443aa9453224f83018bec35f8266bc77915`), the three weights the UI uses (400, `font-medium` and `font-semibold`), as three `@font-face` rules of family `'Geist'` in place of the variable one, with the Regular file preloaded; that changes 2.16.1, 2.16.7, U-202, U-203 and E-62 and needs the same approval. The JetBrains Mono file is unhinted too, but code is a small part of the screen; the same check covers it.
34. **A number and its unit can wrap apart in messages (needs the core and errors.md).** The review of ADR 0031's fonts (2026-10-16) found sentences breaking between a number and its unit: the fillet message "... Try 9.9 mm or less." wraps after "9.9" in its notification and in the inspector's banner and `field-error` at some widths. Those sentences come from the core (features.md sections 4.1 and 4.3), so the fix is to join a number and its unit with a no-break space (U+00A0) where the core writes display text, and where the UI's display-only formatters (`formatQuantity`, `formatSize` and `formatPoint` in `src/expression/format.ts`) feed sentences, while text that goes back into a field (`formatEvaluated`, the "Add parameter {name}" draft, a fix's value) keeps a plain space unless the expression parser accepts U+00A0 (expressions.md). It waits until reference stability phase 1 releases `crates/`, and comes with a rule in the message rules of errors.md and in the units rule of 2.16.7 ("a unit never wraps away from its number"). Vitest checks of exact text and `data-value` attributes (E-140) change with it; Playwright's `toHaveText` already treats U+00A0 as a space.
35. **Selected and pressed states in forced colors.** Resolved by ADR 0032. Forced-colors mode replaces every background with the system's Canvas color (3.24), so the states shown by a fill alone looked like their neighbors there. Every selected, pressed or active element (selected rows and timeline items, the print check's focused finding, pressed toggles and the active tool, the palette's active option and the keyboard-active item of a menu or list) now adds the checked segment's transparent inner outline in that state (3.23), which only forced colors draw; U-213 and E-146 check it.
36. **A wrong file under an asset's name (needs the engine).** The core refuses a whole document when a supplied asset does not hash to its name (`asset_mismatch`, engine.md 3.14), and its text names the asset by its hash, which errors.md keeps out of messages. So the UI opens the text again without assets and says so (`assets_mismatch`, 3.19, ADR 0037 decision 8). It cannot say which file is wrong, and it cannot repair the file, since the core then holds none of the bytes; Python's save repairs it (python-api.md 3.1). Opening with the other assets, or naming the file by the import that uses it, needs the engine to report the asset's name in a field of the error or to keep the other assets (a protocol change).
37. **The web asks for a document's folder.** The File System Access API gives a page a file without its folder. So a document with imports asks for its folder once per document session that holds none, and Save as into another folder asks again (3.19); a recent entry and a recovery copy keep the folder they had. Choosing the folder reopens the document, as a core restart does, which ends its undo history; a protocol command that adds assets to an open document would keep it (ADR 0037, alternatives). Chromium's save dialog creates the file it names and, as we recall from Chromium's code (not tested here, open issue 39), empties it when it exists, so a Save as over an existing file that waits for its folder and is dismissed can leave that file empty; ADR 0037 lists writing the document at once in that case as an option for the gate. Chromium's directory dialog is believed to refuse the home folder itself (and perhaps Desktop, Documents and Downloads) while allowing their subfolders, so a document saved directly there cannot get its imports on the web until it and its assets folder move into a folder of their own (open issue 39). A dropped file carries no handle; Chromium's `DataTransferItem.getAsFileSystemHandle` could give it one later. Browsers without the File System Access API could read a folder through `<input type="file" webkitdirectory>`, read only, and a `.ptsz` bundle (io.md, Later) would carry the assets inside the file; ADR 0037 lists both under alternatives.
38. **The change check is not atomic.** Save reads the file, compares it, then writes the assets and the document (3.19), so a change made elsewhere between that read and the write is still overwritten. A compare and write in the host would shrink that gap on the desktop only, since the web has no conditional write (ADR 0037, options). The check also costs one read of the whole file per save, which no budget measures yet.
39. **What the end-to-end tests cannot show (human check at the gate).** E-150 to E-154 run on the origin private file system, whose handles Chromium answers every permission query for with `granted`, behind pickers that `useFolders` replaces (6.3), and they run in Chromium on Linux only, the one engine with the File System Access API (6.8). So a reviewer checks by hand, in Chrome and Edge with folders picked from the local disk: the permission prompts after a reload (Open recent of a document with imports, then Save), how many there are and that the folder's prompt comes first and covers the file (2.15 `readHandle`), and that a refused folder leads to `assets_folder_needed` (3.19); `resolve` on handles picked from local folders (`folderHolds`); whether `showDirectoryPicker` refuses the home folder itself and which other well-known folders it refuses (open issue 37); and Chromium's save dialog on an existing file, whether it empties the file before the page writes it (open issue 37). The reviewer records each result.
40. **No check for the minimum versions.** The unsupported page (3.1 step 1) checks only WebAssembly and WebGL2, which every browser has from long before the minimums of 1.1. A browser that has both but is older than its minimum, such as Firefox 115 ESR or Safari 16, opens the app without the Popover API, so its menus, popovers and tooltips cannot open (2.10, 3.2). A third check in step 1, `typeof HTMLElement.prototype.showPopover === 'function'`, with its own text in 4.3 in the words of `unsupported_wasm` (for example "Partsmith needs popovers to show its menus, and this version of your browser does not provide them. Update it, or use a current version of Chrome, Edge, Firefox or Safari."), would say so; it waits for a UI change of 3.1 with its E-test, since no browser that CI runs lacks the API. Until then the minimums are documented only, here and in README.md (delivery.md 2.12).
41. **What the engines have shown, and WebKit on macOS.** The first platforms runs (delivery.md 2.14) are the first time the app runs in Firefox and WebKit, so each test of the engine subset that fails in one engine is recorded here with the engine, the test and what it found, until the app or the test is fixed (6.8), and so is an engine that needed to run headed under `xvfb-run` because it had no WebGL2 when headless on the runner. The `webkit` project runs Playwright's WebKit on Linux, the port nearest to WebKitGTK; WKWebView, the desktop webview on macOS, is Apple's port of the same engine, with its own graphics and input layers, and Playwright's WebKit for macOS is the nearest to it. Running the subset there (a `webkit` row on `macos-15`) needs its tests to hold with Meta as `Mod`, since 6.5 assumes Linux, where `Mod` is Control (the shortcut texts that name `Mod`, and the inference hold key of 3.16.5). Until then the desktop checks at the gate cover WKWebView (desktop.md 6). The first run (run 40315773128, scheduled, commit 2a202ac) recorded: **firefox** (job 121549929858): 22 of the 24 tests failed, 21 at their 30 s timeout waiting for `app`, which never rendered, and E-147 with no WebGL2 context ("createWebGl2Backend returned null"), so Firefox showed the unsupported page (3.1). The row's image had neither `libegl1` nor `libegl-mesa0` (the `webkit` row's `playwright install --with-deps webkit` installed both). The project now runs headed under `xvfb-run` (6.8); if E-147 still fails, its message, which now carries the context creation error, decides whether the row needs those packages (delivery.md 2.14). Nothing after startup has run in Firefox yet. **webkit** (job 121549929940): 19 passed; E-8, E-9, E-127 and E-56 found the core `failed` right after `page.reload()`, and `vite preview` answered revalidations with a 304 without COOP and COEP (Vite's static server sends it before its headers); that WebKit refused the core worker for it is the likely cause, unconfirmed, since the log does not show the core's fatal text (`expectReady` now reports it, 6.3). E-144 counted two Geist requests while Vite's CORS middleware sent `Vary: Origin` (the test now records whether each request sent `Origin`). Both server defects are fixed in `vite.config.ts` (delivery.md 2.7); the later steps of those four tests (Open recent, the recovery offers, Theme Light and System) have not run in WebKit yet. Until the next run, each stays recorded here. The second run (run 40478176115, dispatched on `main`, commit 5153d61) passed all 24 tests in both engines, each in 1.3 min: **firefox** (job 122103654212) headed under `xvfb-run` with no package added, so the EGL packages of delivery.md 2.14 stay out; **webkit** (job 122103654202) with E-8, E-9, E-127 and E-56 through their reloads and later steps, and E-144 with one Geist request. It is each project's first pass on `main`; a second makes the rows eligible for promotion into `ci.yml` (delivery.md 2.14). What stays open here is WebKit on macOS (WKWebView), above.
