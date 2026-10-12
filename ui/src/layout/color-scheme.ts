import type { Settings } from '../settings/settings'
import { resolveTheme } from '../theme/theme'
import type { ThemeName } from '../theme/roles'

// The operating system's color scheme, which the theme setting `system` follows (ui.md 2.16.8).

export const DARK_SCHEME_QUERY = '(prefers-color-scheme: dark)'

/** Whether the operating system asks for a dark color scheme; false where it cannot be read. */
export function systemPrefersDark(): boolean {
  return typeof matchMedia === 'function' && matchMedia(DARK_SCHEME_QUERY).matches
}

/** The theme shown for the settings now. */
export function resolvedTheme(settings: Settings): ThemeName {
  return resolveTheme(settings.theme, systemPrefersDark())
}
