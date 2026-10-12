import { useSyncExternalStore } from 'react'
import { useAppState } from '../app/AppProvider'
import type { ThemeName } from '../theme/roles'
import { resolveTheme } from '../theme/theme'
import { DARK_SCHEME_QUERY, systemPrefersDark } from './color-scheme'

// The theme the chrome shows, following the operating system while the setting is `system`
// (ui.md 2.16.8).

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
