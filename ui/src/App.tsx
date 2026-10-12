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
import { SketchHeader } from './sketch/SketchHeader'
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
  useEnterLadder(app)
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
