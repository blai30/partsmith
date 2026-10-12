import { PALETTE_RECENT_LIMIT, saveSettings, type Settings } from '../../settings/settings'
import { browserStorage } from '../../state/store'
import type { App } from '../createApp'

// Settings changes (ui.md 2.15): every change is applied and saved at once.

/** Applies a change to the settings and stores them under `partsmith.settings.v1`. */
export function updateSettings(app: App, change: Partial<Settings>): void {
  const settings = { ...app.store.getState().settings, ...change }
  app.store.setState({ settings })
  saveSettings(browserStorage(), settings)
}

/** Records a palette item id at the front of `settings.paletteRecent`, once (3.4). */
export function recordPaletteRecent(app: App, id: string): void {
  const recent = app.store.getState().settings.paletteRecent
  updateSettings(app, {
    paletteRecent: [id, ...recent.filter((other) => other !== id)].slice(0, PALETTE_RECENT_LIMIT),
  })
}
