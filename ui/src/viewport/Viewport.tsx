import { clsx } from 'clsx/lite'
import { LoaderCircle } from 'lucide-react'
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { useShallow } from 'zustand/react/shallow'
import { useApp, useAppState } from '../app/AppProvider'
import { canStopRunningCore } from '../app/connection'
import { STILL_WORKING_SECONDS, type App } from '../app/createApp'
import { reportFailure } from '../app/report'
import { formatShortcut } from '../commands/shortcuts'
import { originPlaneView } from '../core/labels'
import { Button } from '../components/Button'
import { ExpressionInput } from '../expression/ExpressionInput'
import { formatNumber } from '../expression/format'
import { MESSAGES } from '../messages'
import { requestUi } from '../layout/requests'
import { markTiming, TIMING_NAMES } from '../performance/marks'
import { focusedFindingPoint } from '../print/health'
import { selectRepairMarkers } from '../state/selectors'
import type { AppState, SectionState, ViewState } from '../state/store'
import { isSessionVisible } from '../state/visibility'
import { AXIS_ROLES, COLOR_CLASSES } from '../theme/roles'
import { createWebGl2Backend } from './backend'
import { framedArea, orbit, type Vector3 } from './camera'
import { createViewportController, type RenderedFrame, type ViewportController } from './controller'
import { keyNavigation } from './navigation'
import { orientElement, visiblePosition } from './overlay'
import { drawnOriginPlanes, originPlaneLabelAnchor, planeExtent } from './planes'
import { createScene, type Scene } from './scene'
import { defaultSectionOffset, type SectionPlane } from './section'
import { ViewCube } from './ViewCube'
import { viewCubeRotorMatrix } from './view-cube'
import { ViewportControls, type SelectionFilterKind } from './ViewportControls'
import { ViewportMenu } from './ViewportMenu'
import { CanvasLayerContext } from './canvas-layer'
import { VIEW_ID } from '../layout/focus'

// The 3D view (ui.md 3.2, 3.12 to 3.15): the canvas and its controller under the islands, the
// overlay layer, and the free-area layer with the view cube, the viewport controls and legend, the
// top-center slot, the inspector and the core's loading, still-working and fatal states. React
// renders the chrome once; the controller draws and moves runtime positions without React
// (ADR 0002).

export type ViewportProps = {
  /** The legend at the bottom of the free area's left column, under the viewport controls (3.14). */
  readonly legend?: ReactNode
  /** Chrome in the free area: the top-center slot's content and the inspector. */
  readonly children?: ReactNode
}

/** The offset text a section starts with: the visible center along the plane normal (3.15). */
function defaultOffsetText(state: AppState, scene: Scene, plane: SectionPlane): string {
  const offset = defaultSectionOffset(plane, scene.visibleBounds(state))
  return formatNumber(offset, 'length', state.document?.recipe.units ?? 'mm')
}

function setSection(app: App, change: (section: SectionState) => SectionState | null): void {
  app.store.setState((state) => {
    const section = state.view.section
    return section === null ? {} : { view: { ...state.view, section: change(section) } }
  })
}

function setView(app: App, change: Partial<ViewState>): void {
  app.store.setState((state) => ({ view: { ...state.view, ...change } }))
}

/** The drawn grid's minor spacing in the document unit, or empty when no grid is drawn (3.12). */
function gridSpacingText(state: AppState): string {
  const spacing = state.view.gridSpacing
  return spacing === null
    ? ''
    : formatNumber(spacing, 'length', state.document?.recipe.units ?? 'mm')
}

const AXIS_LABELS = ['X', 'Y', 'Z'] as const

/**
 * The letters at the positive ends of the visible origin axes, in their axis roles on a raised
 * chip, anchored in the overlay (3.12); a sketch being edited labels its own axes (3.16.2).
 */
function AxisLabels({ scene }: { readonly scene: Scene }) {
  // Compared by value: the selectors run on every store change.
  const axes = useAppState(
    useShallow((state) =>
      state.context === 'sketch' && state.sketch?.sketchId !== null
        ? []
        : AXIS_LABELS.filter((axis) => isSessionVisible(state, `origin:${axis}`)),
    ),
  )
  // The bounds are needed only for shown axes.
  const half = useAppState((state) =>
    axes.length === 0 ? 0 : planeExtent(scene.visibleBounds(state)).side / 2,
  )
  return axes.map((axis) => {
    const reach = String(half)
    const anchor = axis === 'X' ? `${reach},0,0` : axis === 'Y' ? `0,${reach},0` : `0,0,${reach}`
    return (
      <span
        key={axis}
        data-testid="axis-label"
        data-axis={axis}
        data-anchor={anchor}
        hidden
        className={clsx(
          'pointer-events-none absolute top-0 left-0 rounded-sm px-1 text-xs font-semibold',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES[AXIS_ROLES[axis]].foreground,
        )}
      >
        {axis}
      </span>
    )
  })
}

/**
 * The labels of the drawn origin planes, "Top (XY)", "Front (XZ)" and "Right (YZ)", at the corner
 * of each plane's square in its positive axis directions, anchored in the overlay (3.16.1, 3.12).
 */
function PlaneLabels({ scene }: { readonly scene: Scene }) {
  // Compared by value: the selectors run on every store change.
  const planes = useAppState(useShallow(drawnOriginPlanes))
  const anchors = useAppState(
    useShallow((state) => {
      const bounds = planes.length === 0 ? null : scene.visibleBounds(state)
      return planes.map((plane) => originPlaneLabelAnchor(plane, bounds).join(','))
    }),
  )
  return planes.map((plane, index) => (
    <span
      key={plane}
      data-testid="plane-label"
      data-anchor={anchors[index]}
      hidden
      className={clsx(
        'pointer-events-none absolute top-0 left-0 rounded-sm px-1 text-xs whitespace-nowrap',
        COLOR_CLASSES.surfaceRaised.background,
        COLOR_CLASSES.text.foreground,
      )}
    >
      {`${originPlaneView(plane)} (${plane})`}
    </span>
  ))
}

/**
 * A card centered in the free area on the `surfaceRaised` role (3.24: text over the view sits on a
 * card), in a layer above the inspector so the core's panels and their buttons stay uncovered (4.4).
 */
function OverlayChip({ children }: { readonly children: ReactNode }) {
  return (
    <div className="pointer-events-none absolute inset-0 z-30 flex items-center justify-center p-4">
      <div
        className={clsx(
          'pointer-events-auto flex max-w-110 flex-col gap-3 rounded-xl border p-4 text-sm shadow-lg',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.border.border,
          COLOR_CLASSES.elevation.shadow,
          COLOR_CLASSES.text.foreground,
        )}
      >
        {children}
      </div>
    </div>
  )
}

function Spinner() {
  return (
    <LoaderCircle
      aria-hidden="true"
      strokeWidth={1.75}
      className={clsx(
        'size-4 shrink-0 animate-spin motion-reduce:animate-none',
        COLOR_CLASSES.textMuted.foreground,
      )}
    />
  )
}

/**
 * A core that has answered nothing for `STILL_WORKING_SECONDS` (4.4), over the model: on the web
 * the user can stop it, while the desktop core cannot be stopped from outside (desktop.md open
 * issue 9).
 */
function StillWorking() {
  const app = useApp()
  const stoppable = canStopRunningCore(app)
  return (
    <OverlayChip>
      <div data-testid="core-working" role="alert" className="flex flex-col gap-3">
        <p>
          {stoppable
            ? MESSAGES.core_still_working(STILL_WORKING_SECONDS)
            : MESSAGES.core_still_working_desktop(STILL_WORKING_SECONDS)}
        </p>
        {stoppable && (
          <div className="flex justify-end">
            <Button
              variant="primary"
              onClick={() => {
                app.restartCore().catch(reportFailure('Stopping and restarting the geometry core'))
              }}
            >
              Stop and restart core
            </Button>
          </div>
        )}
      </div>
    </OverlayChip>
  )
}

/** The loading, still-working, fatal and context-loss states drawn over the view (3.1, 3.12, 4.4). */
function CoreStatus({ restoring }: { readonly restoring: boolean }) {
  const app = useApp()
  const status = useAppState((state) => state.core.status)
  const fatal = useAppState((state) => state.core.fatal)
  const started = useAppState((state) => state.core.engineVersion !== null)
  const stillWorking = useAppState((state) => state.core.stillWorking)
  if (status === 'loading') {
    return (
      <OverlayChip>
        <output data-testid="core-overlay" className="flex items-center gap-3">
          <Spinner />
          Loading the geometry core
        </output>
      </OverlayChip>
    )
  }
  if (status === 'failed') {
    const reason = fatal ?? ''
    return (
      <OverlayChip>
        <div data-testid="core-fatal" role="alert" className="flex flex-col gap-3">
          <p>{started ? MESSAGES.core_stopped(reason) : MESSAGES.core_load_failed(reason)}</p>
          <div className="flex justify-end">
            {started ? (
              <Button
                variant="primary"
                onClick={() => {
                  app.restartCore().catch(reportFailure('Restarting the geometry core'))
                }}
              >
                Restart core
              </Button>
            ) : (
              <Button
                variant="primary"
                onClick={() => {
                  location.reload()
                }}
              >
                Reload
              </Button>
            )}
          </div>
        </div>
      </OverlayChip>
    )
  }
  if (stillWorking) {
    return <StillWorking />
  }
  if (restoring) {
    return (
      <OverlayChip>
        <div
          // ui.md 3.12 names the status role of this overlay.
          // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
          role="status"
          className="flex items-center gap-3"
        >
          <Spinner />
          Restoring the 3D view
        </div>
      </OverlayChip>
    )
  }
  return null
}

/** The section popover's Offset field: an expression in the document's length unit (3.15). */
function SectionOffset({ scene }: { readonly scene: Scene }) {
  const app = useApp()
  const offset = useAppState((state) => state.view.section?.offset ?? '')
  return (
    <ExpressionInput
      id="section-offset"
      label="Offset"
      unit="length"
      value={offset}
      onChange={(value, state) => {
        if (state.state === 'valid') {
          scene.setSectionOffset(value, state.value)
          app.viewport()?.requestRender()
        }
        if (value !== offset) {
          setSection(app, (section) => ({ ...section, offset: value }))
        }
      }}
    />
  )
}

/** The finding marker's radius in CSS pixels: a ring 16 pixels across (3.17). */
const FINDING_MARKER_RADIUS = 8

/** Moves each repair marker over its point, or hides it behind the eye or outside the free area (3.7). */
function placeRepairMarkers(overlay: HTMLElement | null, frame: RenderedFrame): void {
  for (const marker of overlay?.querySelectorAll<SVGCircleElement>(
    '[data-testid="repair-marker"]',
  ) ?? []) {
    const point: Vector3 = [
      Number(marker.dataset['x']),
      Number(marker.dataset['y']),
      Number(marker.dataset['z']),
    ]
    placeFindingMarker(marker, point, frame)
  }
}

/** Apps whose first usable frame has been marked as `ps:usable`. */
const usableMarked = new WeakSet<App>()

/** Moves the finding marker over its point, or hides it behind the eye or outside the free area (3.12). */
function placeFindingMarker(
  marker: SVGCircleElement | null,
  point: Vector3 | null,
  frame: RenderedFrame,
): void {
  if (marker === null) {
    return
  }
  const position = visiblePosition(
    point === null ? null : frame.project(point),
    framedArea(frame.view),
  )
  if (position === null) {
    marker.setAttribute('display', 'none')
    return
  }
  marker.setAttribute('cx', String(position.x))
  marker.setAttribute('cy', String(position.y))
  marker.removeAttribute('display')
}

export function Viewport({ legend = null, children }: ViewportProps) {
  const app = useApp()
  const region = useRef<HTMLElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const surface = useRef<HTMLDivElement>(null)
  const overlay = useRef<HTMLDivElement>(null)
  const [canvasLayer, setCanvasLayer] = useState<HTMLDivElement | null>(null)
  const rotor = useRef<HTMLDivElement>(null)
  const controller = useRef<ViewportController | null>(null)
  const [scene, setScene] = useState<Scene | null>(null)
  const [restoring, setRestoring] = useState(false)

  const named = useAppState((state) => state.view.named)
  const projection = useAppState((state) => state.view.projection)
  const navigation = useAppState((state) => state.settings.navigation)
  const section = useAppState((state) => state.view.section)
  const filter = useAppState((state) => state.view.filter)
  const sketchSession = useAppState((state) => state.context === 'sketch' && state.sketch !== null)
  const hoverKind = useAppState((state) => state.hover?.kind ?? 'none')
  const hoverKey = useAppState((state) => state.hover?.key ?? null)
  const selectionCount = useAppState((state) => state.selection.length)
  const gridSpacing = useAppState(gridSpacingText)
  const findingPoint = useAppState(focusedFindingPoint)
  const repairMarkers = useAppState(selectRepairMarkers)
  // The controller draws a frame when the print state changes, which places the marker (3.17).
  const findingMarker = useRef<SVGCircleElement>(null)
  const platform = app.platform

  useEffect(() => {
    const canvasElement = canvas.current
    const overlayElement = overlay.current
    const freeArea = region.current
    if (canvasElement === null || overlayElement === null || freeArea === null) {
      return undefined
    }
    const backend = createWebGl2Backend(canvasElement)
    if (backend === null) {
      return undefined
    }
    const created = createScene(backend)
    const viewport = createViewportController({
      app,
      canvas: canvasElement,
      overlay: overlayElement,
      freeArea,
      backend,
      scene: created,
    })
    controller.current = viewport
    const stopLost = backend.onContextLost(() => {
      setRestoring(true)
    })
    const stopRestored = backend.onContextRestored(() => {
      setRestoring(false)
    })
    const stopRender = viewport.onRender((frame) => {
      if (rotor.current !== null) {
        orientElement(rotor.current, viewCubeRotorMatrix(frame.camera.rotation))
      }
      const state = app.store.getState()
      placeFindingMarker(findingMarker.current, focusedFindingPoint(state), frame)
      placeRepairMarkers(overlayElement, frame)
      // The first frame with the startup document makes the app usable (3.1).
      if (!usableMarked.has(app) && state.core.status === 'ready' && state.document !== null) {
        usableMarked.add(app)
        markTiming(TIMING_NAMES.usable)
      }
    })
    app.attachViewport(viewport)
    // The scene is created with the canvas, so the chrome that needs it renders after mounting.
    setScene(created)
    return () => {
      stopRender()
      stopLost()
      stopRestored()
      if (app.viewport() === viewport) {
        app.attachViewport(null)
      }
      controller.current = null
      viewport.dispose()
      backend.dispose()
    }
  }, [app])

  // New repair markers are placed by the next frame.
  useEffect(() => {
    if (repairMarkers.length > 0) {
      app.viewport()?.requestRender()
    }
  }, [app, repairMarkers])

  // A section turned on without an offset starts at the visible center (3.15).
  useEffect(() => {
    if (scene !== null && section !== null && section.offset === '') {
      const text = defaultOffsetText(app.store.getState(), scene, section.plane)
      setSection(app, (current) => ({ ...current, offset: text }))
    }
  }, [app, scene, section])

  /** Opens the context menu from the keyboard: at the center of the selection on screen, or of the free area (3.13). */
  function openMenuFromKeyboard(): void {
    const handle = app.viewport()
    if (handle === null) {
      return
    }
    const state = app.store.getState()
    const items = state.context === 'sketch' ? (state.sketch?.selection ?? []) : state.selection
    const rectangle = items.length === 0 ? null : handle.screenRectangle(items)
    const center = rectangle ?? framedArea(handle.viewWindow())
    requestUi(app, {
      kind: 'viewport_menu',
      x: center.x + center.width / 2,
      y: center.y + center.height / 2,
    })
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    // Enter confirms a command through the app's Enter ladder (3.5); here are the camera keys.
    if (event.target !== event.currentTarget || event.defaultPrevented) {
      return
    }
    if ((event.key === 'F10' && event.shiftKey) || event.key === 'ContextMenu') {
      event.preventDefault()
      openMenuFromKeyboard()
      return
    }
    const step = keyNavigation(event.key, {
      shift: event.shiftKey,
      control: event.ctrlKey,
      meta: event.metaKey,
      alt: event.altKey,
    })
    if (step !== null) {
      event.preventDefault()
      controller.current?.navigate(step)
    }
  }

  function toggleSection(): void {
    const current = app.store.getState().view.section
    setView(app, { section: current === null ? { plane: 'XY', offset: '', flipped: false } : null })
  }

  return (
    // The free area (3.2): the grid cell the islands leave. It is not positioned, so the canvas,
    // the overlay layer, the canvas layer and the menu anchor take the app root as their
    // containing block and fill the window under the islands, in canvas pixels.
    <main
      ref={region}
      aria-label="3D view"
      data-testid="viewport"
      data-view={named}
      data-projection={projection}
      data-nav-preset={navigation}
      data-section={section === null ? 'off' : 'on'}
      data-hover-kind={hoverKind}
      data-hover-key={hoverKey ?? undefined}
      data-selection-count={selectionCount}
      data-grid-spacing={gridSpacing}
      className="col-start-2 row-start-2 min-h-0 min-w-0"
    >
      {/* The spec makes the view one focusable application that takes the camera keys (3.13). */}
      {/* Its focus ring is drawn around the free area by the free-area layer (3.23). */}
      {/* oxlint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        ref={surface}
        id={VIEW_ID}
        role="application"
        aria-roledescription="3D view"
        aria-label="3D view"
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        onKeyDown={onKeyDown}
        className="peer absolute inset-0 outline-none"
      >
        <canvas ref={canvas} className="absolute inset-0 block size-full touch-none" />
      </div>
      <div ref={overlay} className="pointer-events-none absolute inset-0 overflow-hidden">
        <svg aria-hidden="true" className="absolute inset-0 size-full">
          {findingPoint === null ? null : (
            <circle
              ref={findingMarker}
              data-testid="finding-marker"
              r={FINDING_MARKER_RADIUS}
              display="none"
              className={clsx('fill-none stroke-2', COLOR_CLASSES.errorMark.stroke)}
            />
          )}
          {repairMarkers.map((marker) => (
            <circle
              key={marker.at.join(',')}
              data-testid="repair-marker"
              data-x={marker.at[0]}
              data-y={marker.at[1]}
              data-z={marker.at[2]}
              r={FINDING_MARKER_RADIUS}
              display="none"
              strokeDasharray="4 3"
              className={clsx('fill-none stroke-2', COLOR_CLASSES.errorMark.stroke)}
            >
              <title>{`Where the missing ${marker.noun} was`}</title>
            </circle>
          ))}
        </svg>
        {scene === null ? null : <AxisLabels scene={scene} />}
        {scene === null ? null : <PlaneLabels scene={scene} />}
      </div>
      <div ref={setCanvasLayer} className="pointer-events-none absolute inset-0" />
      <ViewportMenu surface={surface} />
      <div
        className={clsx(
          'pointer-events-none relative isolate size-full rounded-xl peer-focus-visible:outline-2 peer-focus-visible:-outline-offset-2',
          COLOR_CLASSES.accent.outline,
        )}
      >
        <ViewCube
          rotorRef={rotor}
          onChooseView={(rotation) => {
            const handle = app.viewport()
            handle?.setCamera({ ...handle.getCamera(), rotation }, { animate: true, record: true })
          }}
          onHome={() => {
            const handle = app.viewport()
            handle?.setStandardView('iso')
            handle?.fit()
          }}
          onOrbit={(deltaX, deltaY) => {
            const handle = app.viewport()
            if (handle !== null) {
              const camera = handle.getCamera()
              handle.setCamera(orbit(camera, deltaX, deltaY, camera.target))
            }
          }}
        />
        <div className="pointer-events-none absolute inset-y-4 left-4 flex flex-col flex-wrap content-start items-start gap-2">
          <ViewportControls
            orthographic={projection === 'orthographic'}
            section={section === null ? null : { plane: section.plane, flipped: section.flipped }}
            filter={sketchSession ? null : filter}
            shortcuts={{
              fit: formatShortcut('F6', platform),
              projection: formatShortcut('Shift+P', platform),
              section: formatShortcut('Shift+X', platform),
            }}
            onFit={() => {
              app.viewport()?.fit()
            }}
            onToggleProjection={() => {
              app.viewport()?.toggleProjection()
            }}
            onToggleSection={toggleSection}
            onToggleFilter={(kind: SelectionFilterKind) => {
              setView(app, { filter: { ...filter, [kind]: !filter[kind] } })
            }}
            onSectionPlaneChange={(plane) => {
              const text =
                scene === null ? '' : defaultOffsetText(app.store.getState(), scene, plane)
              setSection(app, (current) => ({ ...current, plane, offset: text }))
            }}
            onSectionFlippedChange={(flipped) => {
              setSection(app, (current) => ({ ...current, flipped }))
            }}
            sectionOffsetField={scene === null ? null : <SectionOffset scene={scene} />}
          />
          {legend}
        </div>
        <CanvasLayerContext value={canvasLayer}>{children}</CanvasLayerContext>
        <CoreStatus restoring={restoring} />
      </div>
    </main>
  )
}
