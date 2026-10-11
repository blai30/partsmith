export type Settings = {
  readonly theme: 'system' | 'light' | 'dark'
  readonly uiScale: 1 | 1.25 | 1.5 | 2 | 3
  readonly navigation: 'fusion' | 'freecad'
  /** Ids of palette items, most recently run first. */
  readonly paletteRecent: readonly string[]
  /** Print checks hidden from the health counts. */
  readonly hiddenChecks: readonly string[]
  /** Whether the inspector's "More options" is open, per feature type. */
  readonly moreExpanded: Readonly<Record<string, boolean>>
}

/** The localStorage key the settings are stored under, as JSON. */
export const SETTINGS_KEY = 'partsmith.settings.v1'

/** The most palette ids `paletteRecent` keeps. */
export const PALETTE_RECENT_LIMIT = 20

export const DEFAULT_SETTINGS: Settings = {
  theme: 'system',
  uiScale: 1,
  navigation: 'fusion',
  paletteRecent: [],
  hiddenChecks: [],
  moreExpanded: {},
}

const THEMES: readonly Settings['theme'][] = ['system', 'light', 'dark']
const UI_SCALES: readonly Settings['uiScale'][] = [1, 1.25, 1.5, 2, 3]
const NAVIGATIONS: readonly Settings['navigation'][] = ['fusion', 'freecad']

function oneOf<Value>(options: readonly Value[], value: unknown, fallback: Value): Value {
  return options.find((option) => option === value) ?? fallback
}

function stringList(value: unknown, fallback: readonly string[]): readonly string[] {
  if (Array.isArray(value) && value.every((item) => typeof item === 'string')) {
    return value
  }
  return fallback
}

function booleanMap(
  value: unknown,
  fallback: Readonly<Record<string, boolean>>,
): Readonly<Record<string, boolean>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return fallback
  }
  const entries = Object.entries(value)
  if (!entries.every(([, flag]) => typeof flag === 'boolean')) {
    return fallback
  }
  return Object.fromEntries(entries) as Record<string, boolean>
}

/** Settings from stored JSON: each missing or invalid member takes its default. */
function settingsFrom(stored: unknown): Settings {
  if (typeof stored !== 'object' || stored === null || Array.isArray(stored)) {
    return DEFAULT_SETTINGS
  }
  const member = (name: keyof Settings): unknown => Reflect.get(stored, name)
  return {
    theme: oneOf(THEMES, member('theme'), DEFAULT_SETTINGS.theme),
    uiScale: oneOf(UI_SCALES, member('uiScale'), DEFAULT_SETTINGS.uiScale),
    navigation: oneOf(NAVIGATIONS, member('navigation'), DEFAULT_SETTINGS.navigation),
    paletteRecent: stringList(member('paletteRecent'), DEFAULT_SETTINGS.paletteRecent).slice(
      0,
      PALETTE_RECENT_LIMIT,
    ),
    hiddenChecks: stringList(member('hiddenChecks'), DEFAULT_SETTINGS.hiddenChecks),
    moreExpanded: booleanMap(member('moreExpanded'), DEFAULT_SETTINGS.moreExpanded),
  }
}

/** Reads the settings; any failure to read or parse them yields the defaults. */
export function loadSettings(storage: Storage | null): Settings {
  if (storage === null) {
    return DEFAULT_SETTINGS
  }
  try {
    const text = storage.getItem(SETTINGS_KEY)
    return text === null ? DEFAULT_SETTINGS : settingsFrom(JSON.parse(text))
  } catch {
    // Blocked storage or malformed JSON: the app runs on defaults.
    return DEFAULT_SETTINGS
  }
}

/** Stores the settings; a storage that fails leaves them in effect for this session only. */
export function saveSettings(storage: Storage | null, settings: Settings): void {
  if (storage === null) {
    return
  }
  try {
    storage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  } catch {
    // Blocked storage or a full quota: the settings stay in memory.
  }
}
