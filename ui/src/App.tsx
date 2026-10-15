import { clsx } from 'clsx/lite'
import { useEffect } from 'react'
import { useApp, useAppState } from './app/AppProvider'
import { Browser } from './browser/Browser'
import { Inspector } from './inspector/Inspector'
import { CommandPalette } from './layout/CommandPalette'
import { DropOverlay } from './layout/DropOverlay'
import { useEnterLadder, useEscapeLadder } from './layout/ladders'
import { useTypedValues } from './layout/use-typed-values'
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
import { PrintHint } from './print/PrintHint'
import { SketchOverlay } from './sketch/SketchOverlay'
import type { AppState } from './state/store'
import { COLOR_CLASSES, FONT_CLASSES, ROOT_CLASSES } from './theme/roles'
import { applyTheme, applyUiScale } from './theme/theme'
import { Timeline } from './timeline/Timeline'
import { EmptyState } from './viewport/EmptyState'
import { Viewport } from './viewport/Viewport'

// The layout shell (ui.md 3.2): the 3D view fills the window, and the top bar, browser, side
// panel, timeline and status bar float over it as islands, each in its own error boundary (4.4),
// plus the palette, the live regions, file drops, the theme on the root element (2.16.8) and the
// Esc and Enter ladders (3.5).

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
  useTypedValues(app)
  // One grid places every region by area, so the DOM, and with it the tab order, follows 3.24:
  // the timeline comes before the side panel, and the notification stack, drawn over the free
  // area's bottom-right, comes last. The grid has no gap: each island keeps 0.5rem from the free
  // area with a margin on the side facing it, and the root's padding keeps it from the window.
  return (
    <div
      data-testid="app"
      data-core-status={status}
      data-context={context}
      {...drop.handlers}
      className={clsx(
        '@container/app relative grid h-dvh w-dvw grid-cols-[auto_minmax(0,1fr)_auto] grid-rows-[auto_minmax(0,1fr)_auto_auto] overflow-hidden p-2 antialiased',
        FONT_CLASSES.interfaceText,
        COLOR_CLASSES.stage.background,
        COLOR_CLASSES.text.foreground,
      )}
    >
      <RegionBoundary
        region="top bar"
        fallbackClassName="relative z-10 col-span-3 row-start-1 mb-2 h-12 rounded-xl border shadow-md"
      >
        <TopBar />
      </RegionBoundary>
      {browserOpen ? (
        <RegionBoundary
          region="browser"
          fallbackClassName="relative z-10 col-start-1 row-start-2 mr-2 w-66 rounded-xl border shadow-md"
        >
          <Browser />
        </RegionBoundary>
      ) : null}
      <RegionBoundary
        region="viewport"
        fallbackClassName="relative col-start-2 row-start-2 rounded-xl border shadow-md"
      >
        <Viewport legend={<PrintLegend />}>
          <EmptyState />
          <SketchOverlay />
          <PrintHint />
          <Inspector />
        </Viewport>
      </RegionBoundary>
      <RegionBoundary
        region="timeline"
        fallbackClassName="relative z-10 col-span-3 row-start-3 mt-2 h-14 rounded-xl border shadow-md"
      >
        <Timeline />
      </RegionBoundary>
      <RegionBoundary
        region="side panel"
        fallbackClassName="relative z-10 col-start-3 row-start-2 ml-2 w-90 rounded-xl border shadow-md"
      >
        <SidePanel />
      </RegionBoundary>
      <RegionBoundary
        region="status bar"
        fallbackClassName="relative z-10 col-span-3 row-start-4 mt-2 h-7 rounded-lg border shadow-md"
      >
        <StatusBar />
      </RegionBoundary>
      <Notifications />
      <CommandPalette />
      <LiveRegions />
      {drop.over ? <DropOverlay /> : null}
    </div>
  )
}
