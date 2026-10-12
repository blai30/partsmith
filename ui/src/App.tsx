import { clsx } from 'clsx/lite'
import { useEffect } from 'react'
import { useApp, useAppState } from './app/AppProvider'
import { Browser } from './browser/Browser'
import { Inspector } from './inspector/Inspector'
import { CommandPalette } from './layout/CommandPalette'
import { DropOverlay } from './layout/DropOverlay'
import { useEnterLadder, useEscapeLadder } from './layout/ladders'
import { LiveRegions } from './layout/LiveRegions'
import { Notifications } from './layout/Notifications'
import { RegionBoundary } from './layout/RegionBoundary'
import { SidePanel } from './layout/SidePanel'
import { StatusBar } from './layout/StatusBar'
import { TopBar } from './layout/TopBar'
import { useFileDrop } from './layout/use-file-drop'
import { useResolvedTheme } from './layout/use-resolved-theme'
import { OverhangLegend } from './print/OverhangLegend'
import { overhangLegendLimit } from './print/overhang'
import { SketchOverlay } from './sketch/SketchOverlay'
import type { AppState } from './state/store'
import { COLOR_CLASSES, FONT_CLASSES, ROOT_CLASSES } from './theme/roles'
import { applyTheme, applyUiScale } from './theme/theme'
import { Timeline } from './timeline/Timeline'
import { Viewport } from './viewport/Viewport'

// The layout shell (ui.md 3.2): top bar, browser, viewport, side panel, timeline and status bar,
// each in its own error boundary (4.4), plus the palette, the live regions, file drops, the theme
// on the root element (2.16.8) and the Esc and Enter ladders (3.5).

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

/** The overhang legend while the overhang map is shown in Print context (3.17). */
function PrintLegend() {
  const limit = useAppState(overhangLegendLimit)
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
  useEnterLadder(app)
  // One grid places every region by area, so the DOM, and with it the tab order, follows 3.24:
  // the timeline comes before the side panel, and the notification stack, drawn over the
  // viewport's bottom-right, comes last.
  return (
    <div
      data-testid="app"
      data-core-status={status}
      data-context={context}
      {...drop.handlers}
      className={clsx(
        'relative grid h-dvh w-dvw grid-cols-[auto_minmax(0,1fr)_auto] grid-rows-[auto_minmax(0,1fr)_auto_auto] overflow-hidden antialiased',
        FONT_CLASSES.interfaceText,
        COLOR_CLASSES.surface.background,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <RegionBoundary region="top bar" fallbackClassName="col-span-3 row-start-1 h-12">
        <TopBar />
      </RegionBoundary>
      {browserOpen ? (
        <RegionBoundary region="browser" fallbackClassName="col-start-1 row-start-2 w-66">
          <Browser />
        </RegionBoundary>
      ) : null}
      <RegionBoundary region="viewport" fallbackClassName="col-start-2 row-start-2">
        <Viewport>
          <SketchOverlay />
          <PrintLegend />
          <Inspector />
        </Viewport>
      </RegionBoundary>
      <RegionBoundary region="timeline" fallbackClassName="col-span-3 row-start-3 h-14">
        <Timeline />
      </RegionBoundary>
      <RegionBoundary region="side panel" fallbackClassName="col-start-3 row-start-2 w-90">
        <SidePanel />
      </RegionBoundary>
      <RegionBoundary region="status bar" fallbackClassName="col-span-3 row-start-4 h-7">
        <StatusBar />
      </RegionBoundary>
      <Notifications />
      <CommandPalette />
      <LiveRegions />
      {drop.over ? <DropOverlay /> : null}
    </div>
  )
}
