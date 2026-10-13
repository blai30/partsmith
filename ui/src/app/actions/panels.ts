import type { Notification, PanelsState } from '../../state/store'
import type { App } from '../createApp'

// Opening and closing the panels around the 3D view (ui.md 3.2): the browser, the timeline, the
// command palette and the side panel.

/** Opens the side panel showing `side`, replacing any panel shown there. */
export function openSidePanel(app: App, side: NonNullable<PanelsState['side']>): void {
  app.store.setState((state) => ({ panels: { ...state.panels, side } }))
}

/** Closes the side panel. */
export function closeSidePanel(app: App): void {
  app.store.setState((state) => ({ panels: { ...state.panels, side: null } }))
}

/** Opens or closes the browser. */
export function setBrowserOpen(app: App, open: boolean): void {
  app.store.setState((state) => ({ panels: { ...state.panels, browserOpen: open } }))
}

/** Opens or closes the command palette. */
export function setPaletteOpen(app: App, open: boolean): void {
  app.store.setState((state) => ({ panels: { ...state.panels, paletteOpen: open } }))
}

/** Flips one of the panel switches. */
export function togglePanel(
  app: App,
  key: 'browserOpen' | 'timelineCollapsed' | 'paletteOpen',
): void {
  app.store.setState((state) => ({ panels: { ...state.panels, [key]: !state.panels[key] } }))
}

/** The "Show problems" action of a notification, which opens the problems panel. */
export function showProblemsAction(app: App): Notification['actions'][number] {
  return {
    label: 'Show problems',
    run: () => {
      openSidePanel(app, 'problems')
    },
  }
}
