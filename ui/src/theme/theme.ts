import type { Settings } from '../settings/settings'
import { UI_SCALE_CLASSES, type ThemeName } from './roles'

const DARK_CLASS = 'dark'

/** The theme to show for a theme setting: 'system' follows the operating system. */
export function resolveTheme(setting: Settings['theme'], prefersDark: boolean): ThemeName {
  if (setting === 'system') {
    return prefersDark ? 'dark' : 'light'
  }
  return setting
}

/** Adds the `dark` class to the root element for 'dark' and removes it for 'light'. */
export function applyTheme(root: HTMLElement, theme: ThemeName): void {
  root.classList.toggle(DARK_CLASS, theme === 'dark')
}

/** Replaces the root element's UI scale class with UI_SCALE_CLASSES[scale]. */
export function applyUiScale(root: HTMLElement, scale: Settings['uiScale']): void {
  root.classList.remove(...Object.values(UI_SCALE_CLASSES))
  root.classList.add(UI_SCALE_CLASSES[scale])
}

/** The explicit theme the toggle switches to: the opposite of the resolved theme. */
export function toggledTheme(resolved: ThemeName): ThemeName {
  return resolved === 'dark' ? 'light' : 'dark'
}
