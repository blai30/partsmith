import { clsx } from 'clsx/lite'
import { useEffect, useRef, useState, type DragEvent } from 'react'
import { cancelCommand } from './app/actions/command'
import { openFile } from './app/actions/document'
import { clearSelection } from './app/actions/selection'
import { finishSketch, setSketchTool } from './app/actions/sketch'
import { useApp, useAppState } from './app/AppProvider'
import type { App as AppHandle } from './app/createApp'
import { Browser } from './browser/Browser'
import { Inspector } from './inspector/Inspector'
import { useResolvedTheme } from './layout/use-resolved-theme'
import { CommandPalette } from './layout/CommandPalette'
import { Notifications } from './layout/Notifications'
import { RegionBoundary } from './layout/RegionBoundary'
import { SidePanel } from './layout/SidePanel'
import { StatusBar } from './layout/StatusBar'
import { TopBar } from './layout/TopBar'
import { OverhangLegend } from './print/OverhangLegend'
import { overhangLegendLimit } from './print/overhang'
import { SketchHeader } from './sketch/SketchHeader'
import type { AppState, SelectionItem } from './state/store'
import { COLOR_CLASSES, FONT_CLASSES, ROOT_CLASSES } from './theme/roles'
import { applyTheme, applyUiScale } from './theme/theme'
import { Timeline } from './timeline/Timeline'
import { Viewport } from './viewport/Viewport'

// The layout shell (ui.md 3.2): top bar, browser, viewport, side panel, timeline and status bar,
// each in its own error boundary (4.4), plus the palette, the live regions, the theme on the root
// element (2.16.8) and the Esc ladder (3.5).

/** `ready` only once the core runs and the first document is open (3.1). */
function selectCoreStatus(state: AppState): AppState['core']['status'] {
  return state.core.status === 'ready' && state.document === null ? 'loading' : state.core.status
}

/** Applies the resolved theme, the UI scale and the root classes to the root element. */
function useRootClasses(): void {
  const theme = useResolvedTheme()
  const scale = useAppState((state) => state.settings.uiScale)
  useEffect(() => {
    document.documentElement.classList.add(...ROOT_CLASSES.split(' '))
  }, [])
  useEffect(() => {
    applyTheme(document.documentElement, theme)
  }, [theme])
  useEffect(() => {
    applyUiScale(document.documentElement, scale)
  }, [scale])
}

/**
 * One step of the Esc ladder after the open layers, which close themselves (3.5): exit the sketch
 * tool, cancel the command, cancel a new sketch's plane pick, then clear the selection. Esc never
 * leaves sketch mode once a sketch is being edited. Returns whether a step applied.
 */
function escapeStep(app: AppHandle): boolean {
  const state = app.store.getState()
  const sketch = state.context === 'sketch' ? state.sketch : null
  if (sketch !== null) {
    if (sketch.sketchId === null) {
      void finishSketch(app)
      return true
    }
    if (sketch.tool !== null) {
      setSketchTool(app, null)
      return true
    }
    if (sketch.selection.length > 0) {
      app.store.setState((current) =>
        current.sketch === null ? {} : { sketch: { ...current.sketch, selection: [] } },
      )
      return true
    }
    return false
  }
  if (state.command !== null) {
    cancelCommand(app).catch((error: unknown) => {
      console.error('Canceling the command failed.', error)
    })
    return true
  }
  if (state.selection.length > 0) {
    clearSelection(app)
    return true
  }
  return false
}

/**
 * Runs the Esc ladder for presses no open layer took. Menus, popovers, the palette and inline
 * editors close on Esc themselves: they either prevent the key's default or are popovers the
 * browser light-dismisses, which are tracked through their toggle events.
 */
function useEscapeLadder(app: AppHandle): void {
  useEffect(() => {
    const openLayers = new Set<Element>()
    let layerOpenAtPress = false
    const onToggle = (event: Event): void => {
      const target = event.target
      // Tooltips are manual popovers: they never take an Esc press from the ladder.
      if (!(target instanceof Element) || target.getAttribute('popover') === 'manual') {
        return
      }
      const newState: unknown = Reflect.get(event, 'newState')
      if (newState === 'open') {
        openLayers.add(target)
      } else if (newState === 'closed') {
        openLayers.delete(target)
      }
    }
    // The capture phase runs before the browser or a popover closes the layer for this press.
    const onKeyDownCapture = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape') {
        return
      }
      for (const layer of openLayers) {
        if (!layer.isConnected) {
          openLayers.delete(layer)
        }
      }
      layerOpenAtPress = openLayers.size > 0
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key !== 'Escape' || event.defaultPrevented || event.isComposing) {
        return
      }
      if (layerOpenAtPress || app.store.getState().panels.paletteOpen) {
        return
      }
      if (escapeStep(app)) {
        event.preventDefault()
      }
    }
    document.addEventListener('toggle', onToggle, true)
    window.addEventListener('keydown', onKeyDownCapture, true)
    window.addEventListener('keydown', onKeyDown)
    return () => {
      document.removeEventListener('toggle', onToggle, true)
      window.removeEventListener('keydown', onKeyDownCapture, true)
      window.removeEventListener('keydown', onKeyDown)
    }
  }, [app])
}

/** How long the selection must stay unchanged before it is announced (3.6). */
const SELECTION_ANNOUNCE_DELAY_MILLISECONDS = 500

function selectionMessage(selection: readonly SelectionItem[]): string {
  const [only] = selection
  if (only === undefined) {
    return 'Selection cleared'
  }
  return selection.length === 1
    ? `${only.label} selected`
    : `${String(selection.length)} items selected`
}

/** The polite live regions: undo and redo announcements, and the debounced selection (3.6, 3.20). */
function LiveRegions() {
  const announcement = useAppState((state) => state.announcement)
  const selection = useAppState((state) => state.selection)
  const [selectionText, setSelectionText] = useState('')
  const first = useRef(true)
  useEffect(() => {
    if (first.current) {
      // The selection the app starts with is not news.
      first.current = false
      return undefined
    }
    const timer = setTimeout(() => {
      setSelectionText(selectionMessage(selection))
    }, SELECTION_ANNOUNCE_DELAY_MILLISECONDS)
    return () => {
      clearTimeout(timer)
    }
  }, [selection])
  return (
    <>
      <div aria-live="polite" className="sr-only">
        {announcement === null ? null : <span key={announcement.id}>{announcement.message}</span>}
      </div>
      <div aria-live="polite" className="sr-only">
        {selectionText}
      </div>
    </>
  )
}

/** Whether a drag carries files, the only thing a drop opens. */
function carriesFiles(event: DragEvent): boolean {
  return event.dataTransfer.types.includes('Files')
}

/** Opening a .ptsm file dropped on the window, with an overlay while it is dragged over (3.19). */
function useFileDrop(app: AppHandle) {
  const [over, setOver] = useState(false)
  // Nested elements fire their own enter and leave events; the depth tells when the window is left.
  const depth = useRef(0)
  return {
    over,
    handlers: {
      onDragEnter: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        event.preventDefault()
        depth.current += 1
        setOver(true)
      },
      onDragOver: (event: DragEvent) => {
        if (carriesFiles(event)) {
          event.preventDefault()
          event.dataTransfer.dropEffect = 'copy'
        }
      },
      onDragLeave: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        depth.current = Math.max(0, depth.current - 1)
        if (depth.current === 0) {
          setOver(false)
        }
      },
      onDrop: (event: DragEvent) => {
        if (!carriesFiles(event)) {
          return
        }
        event.preventDefault()
        depth.current = 0
        setOver(false)
        const [file] = event.dataTransfer.files
        if (file === undefined) {
          return
        }
        file.arrayBuffer().then(
          (buffer) =>
            openFile(app, { name: file.name, bytes: new Uint8Array(buffer), handle: null }),
          (error: unknown) => {
            console.error(`Reading ${file.name} failed.`, error)
          },
        )
      },
    },
  }
}

function DropOverlay() {
  return (
    <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center p-8">
      <div
        data-testid="drop-overlay"
        className={clsx(
          'rounded-md border-2 border-dashed px-8 py-6 text-base font-semibold shadow-lg',
          COLOR_CLASSES.surfaceRaised.background,
          COLOR_CLASSES.accent.border,
          COLOR_CLASSES.text.foreground,
        )}
      >
        Drop a .ptsm file to open it
      </div>
    </div>
  )
}

function PrintLegend() {
  const limit = useAppState((state) =>
    state.context === 'print' ? overhangLegendLimit(state) : null,
  )
  return limit === null ? null : <OverhangLegend limit={limit} />
}

export function App() {
  const app = useApp()
  const status = useAppState(selectCoreStatus)
  const context = useAppState((state) => state.context)
  const browserOpen = useAppState((state) => state.panels.browserOpen)
  const drop = useFileDrop(app)
  useRootClasses()
  useEscapeLadder(app)
  return (
    <div
      data-testid="app"
      data-core-status={status}
      data-context={context}
      {...drop.handlers}
      className={clsx(
        'flex h-dvh w-dvw flex-col overflow-hidden antialiased',
        FONT_CLASSES.interfaceText,
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <RegionBoundary region="top bar" fallbackClassName="h-12 shrink-0">
        <TopBar />
      </RegionBoundary>
      <div className="relative flex min-h-0 flex-1">
        {browserOpen ? (
          <RegionBoundary region="browser" fallbackClassName="w-66 shrink-0">
            <Browser />
          </RegionBoundary>
        ) : null}
        <RegionBoundary region="viewport" fallbackClassName="flex-1">
          <Viewport>
            <SketchHeader />
            <PrintLegend />
            <Inspector />
            <Notifications />
          </Viewport>
        </RegionBoundary>
        <RegionBoundary region="side panel" fallbackClassName="w-90 shrink-0">
          <SidePanel />
        </RegionBoundary>
      </div>
      <RegionBoundary region="timeline" fallbackClassName="h-14 shrink-0">
        <Timeline />
      </RegionBoundary>
      <RegionBoundary region="status bar" fallbackClassName="h-7 shrink-0">
        <StatusBar />
      </RegionBoundary>
      <CommandPalette />
      <LiveRegions />
      {drop.over ? <DropOverlay /> : null}
    </div>
  )
}
