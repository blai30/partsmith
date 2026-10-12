import { saveSettings, type Settings } from '../../settings/settings'
import { browserStorage } from '../../state/store'
import type { App } from '../createApp'

// Settings changes (ui.md 2.15): every change is applied and saved at once.

/** Applies a change to the settings and stores them under `partsmith.settings.v1`. */
export function updateSettings(app: App, change: Partial<Settings>): void {
  const settings = { ...app.store.getState().settings, ...change }
  app.store.setState({ settings })
  saveSettings(browserStorage(), settings)
}
