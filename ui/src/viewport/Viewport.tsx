import { clsx } from 'clsx/lite'
import { LoaderCircle } from 'lucide-react'
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react'
import { confirmCommand } from '../app/actions/command'
import { useApp, useAppState } from '../app/AppProvider'
import type { App } from '../app/createApp'
import { formatShortcut } from '../commands/shortcuts'
import { Button } from '../components/Button'
import { ExpressionInput } from '../expression/ExpressionInput'
import { formatNumber } from '../expression/format'
import { MESSAGES } from '../messages'
import type { AppState, SectionState, ViewState } from '../state/store'
import { COLOR_CLASSES } from '../theme/roles'
import { createWebGl2Backend } from './backend'
import { orbit } from './camera'
import { createViewportController, type ViewportController } from './controller'
import { keyNavigation } from './navigation'
import { orientElement } from './overlay'
import { isSessionVisible, planeExtent, createScene, type Scene } from './scene'
import { defaultSectionOffset, type SectionPlane } from './section'
import { ViewCube } from './ViewCube'
import { viewCubeRotorMatrix } from './view-cube'
import { ViewportControls, type SelectionFilterKind } from './ViewportControls'

// The 3D view (ui.md 3.2, 3.12 to 3.15): the canvas and its controller, the overlay layer, the
// view cube and viewport controls, and the core's loading and fatal states. React renders the
// chrome once; the controller draws and moves runtime positions without React (ADR 0002).

export type ViewportProps = {
  /** Chrome over the view: the inspector, notifications, the sketch header and legends. */
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

const AXIS_LABELS = ['X', 'Y', 'Z'] as const

/** The axis labels at the positive ends of the visible origin axes, anchored in the overlay (3.12). */
function AxisLabels({ scene }: { readonly scene: Scene }) {
  const labels = useAppState((state) => {
    if (state.context === 'sketch' && state.sketch?.sketchId !== null) {
      return ''
    }
    const half = planeExtent(scene.visibleBounds(state)).side / 2
    return AXIS_LABELS.filter((axis) => isSessionVisible(state, `origin:${axis}`))
      .map((axis) => `${axis}:${String(half)}`)
      .join(' ')
  })
  if (labels === '') {
    return null
  }
  return labels.split(' ').map((entry) => {
    const [axis = 'X', reach = '0'] = entry.split(':')
    const anchor = axis === 'X' ? `${reach},0,0` : axis === 'Y' ? `0,${reach},0` : `0,0,${reach}`
    return (
      <span
        key={axis}
        data-anchor={anchor}
        hidden
        className={clsx(
          'absolute top-0 left-0 rounded-sm px-1 text-xs font-medium',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.textMuted.foreground,
        )}
      >
        {axis}
      </span>
    )
  })
}

/** The loading, fatal and context-loss states drawn over the view (3.1, 3.12, 4.4). */
function CoreStatus({ restoring }: { readonly restoring: boolean }) {
  const app = useApp()
  const status = useAppState((state) => state.core.status)
  const fatal = useAppState((state) => state.core.fatal)
  const started = useAppState((state) => state.core.engineVersion !== null)
  const chip = clsx(
    'pointer-events-auto flex max-w-110 flex-col gap-3 rounded-md border p-4 text-sm shadow-lg',
    COLOR_CLASSES.surfaceRaised.background,
    COLOR_CLASSES.border.border,
    COLOR_CLASSES.text.foreground,
  )
  if (status === 'loading') {
    return (
      <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
        <div data-testid="core-overlay" role="status" className={clsx(chip, 'flex-row items-center')}>
          <LoaderCircle
            aria-hidden="true"
            strokeWidth={1.75}
            className={clsx('size-4 animate-spin motion-reduce:animate-none', COLOR_CLASSES.textMuted.foreground)}
          />
          Loading the geometry core
        </div>
      </div>
    )
  }
  if (status === 'failed') {
    const reason = fatal ?? ''
    return (
      <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-4">
        <div data-testid="core-fatal" role="alert" className={chip}>
          <p>{started ? MESSAGES.core_stopped(reason) : MESSAGES.core_load_failed(reason)}</p>
          <div className="flex justify-end">
            {started ? (
              <Button
                variant="primary"
                onClick={() => {
                  app.restartCore().catch((error: unknown) => {
                    console.error('Restarting the geometry core failed.', error)
                  })
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
      </div>
    )
  }
  if (restoring) {
    return (
      <div className="pointer-events-none absolute inset-0 flex items-center justify-center">
        <div role="status" className={clsx(chip, 'flex-row items-center')}>
          <LoaderCircle
            aria-hidden="true"
            strokeWidth={1.75}
            className={clsx('size-4 animate-spin motion-reduce:animate-none', COLOR_CLASSES.textMuted.foreground)}
          />
          Restoring the 3D view
        </div>
      </div>
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

export function Viewport({ children }: ViewportProps) {
  const app = useApp()
  const canvas = useRef<HTMLCanvasElement>(null)
  const overlay = useRef<HTMLDivElement>(null)
  const rotor = useRef<HTMLDivElement>(null)
  const controller = useRef<ViewportController | null>(null)
  const [scene, setScene] = useState<Scene | null>(null)
  const [restoring, setRestoring] = useState(false)

  const named = useAppState((state) => state.view.named)
  const projection = useAppState((state) => state.view.projection)
  const navigation = useAppState((state) => state.settings.navigation)
  const section = useAppState((state) => state.view.section)
  const filter = useAppState((state) => state.view.filter)
  const sketchMode = useAppState((state) => state.context === 'sketch' && state.sketch !== null)
  const hoverKind = useAppState((state) => state.hover?.kind ?? 'none')
  const hoverKey = useAppState((state) => state.hover?.key ?? null)
  const selectionCount = useAppState((state) => state.selection.length)
  const platform = app.platform

  useEffect(() => {
    const canvasElement = canvas.current
    const overlayElement = overlay.current
    if (canvasElement === null || overlayElement === null) {
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

  // A section turned on without an offset starts at the visible center (3.15).
  useEffect(() => {
    if (scene !== null && section !== null && section.offset === '') {
      const text = defaultOffsetText(app.store.getState(), scene, section.plane)
      setSection(app, (current) => ({ ...current, offset: text }))
    }
  }, [app, scene, section])

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.target !== event.currentTarget || event.defaultPrevented) {
      return
    }
    if (event.key === 'Enter' && app.store.getState().command !== null) {
      // The Enter ladder: confirm the command from the focused viewport (3.5).
      event.preventDefault()
      confirmCommand(app).catch((error: unknown) => {
        console.error('Confirming the command failed.', error)
      })
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
    <main
      aria-label="3D view"
      data-testid="viewport"
      data-view={named}
      data-projection={projection}
      data-nav-preset={navigation}
      data-section={section === null ? 'off' : 'on'}
      data-hover-kind={hoverKind}
      data-hover-key={hoverKey ?? undefined}
      data-selection-count={selectionCount}
      className="relative min-h-0 min-w-0 flex-1 overflow-hidden"
    >
      <div
        role="application"
        aria-roledescription="3D view"
        aria-label="3D view"
        tabIndex={0}
        onKeyDown={onKeyDown}
        className={clsx(
          'absolute inset-0 focus-visible:outline-2 focus-visible:-outline-offset-2',
          COLOR_CLASSES.accent.outline,
        )}
      >
        <canvas ref={canvas} className="absolute inset-0 block size-full touch-none" />
      </div>
      <div
        ref={overlay}
        className="pointer-events-none absolute inset-0 overflow-hidden"
      >
        <svg aria-hidden="true" className="absolute inset-0 size-full" />
        {scene === null ? null : <AxisLabels scene={scene} />}
      </div>
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
      <ViewportControls
        orthographic={projection === 'orthographic'}
        section={section === null ? null : { plane: section.plane, flipped: section.flipped }}
        filter={sketchMode ? null : filter}
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
          const text = scene === null ? '' : defaultOffsetText(app.store.getState(), scene, plane)
          setSection(app, (current) => ({ ...current, plane, offset: text }))
        }}
        onSectionFlippedChange={(flipped) => {
          setSection(app, (current) => ({ ...current, flipped }))
        }}
        sectionOffsetField={scene === null ? null : <SectionOffset scene={scene} />}
      />
      <CoreStatus restoring={restoring} />
      {children}
    </main>
  )
}
