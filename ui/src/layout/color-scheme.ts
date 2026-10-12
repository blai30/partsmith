import { useSyncExternalStore } from 'react'
import { useAppState } from '../app/AppProvider'
import type { Settings } from '../settings/settings'
import type { ThemeName } from '../theme/roles'
import { resolveTheme } from '../theme/theme'

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

function subscribeToScheme(listener: () => void): () => void {
  if (typeof matchMedia !== 'function') {
    return () => undefined
  }
  const query = matchMedia(DARK_SCHEME_QUERY)
  query.addEventListener('change', listener)
  return () => {
    query.removeEventListener('change', listener)
  }
}

/** The theme shown now, following the operating system while the setting is `system`. */
export function useResolvedTheme(): ThemeName {
  const setting = useAppState((state) => state.settings.theme)
  const prefersDark = useSyncExternalStore(subscribeToScheme, systemPrefersDark, () => false)
  return resolveTheme(setting, prefersDark)
}
