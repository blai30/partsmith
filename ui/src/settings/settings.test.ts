import { afterEach, describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS, loadSettings, saveSettings, type Settings } from './settings'

const KEY = 'partsmith.settings.v1'

// A Storage whose every access throws, as in a browser that blocks site data.
function blockedStorage(): Storage {
  const blocked = (): never => {
    throw new DOMException('The operation is insecure.', 'SecurityError')
  }
  return {
    get length(): number {
      return blocked()
    },
    clear: blocked,
    getItem: blocked,
    key: blocked,
    removeItem: blocked,
    setItem: blocked,
  }
}

afterEach(() => {
  localStorage.clear()
})

describe('DEFAULT_SETTINGS', () => {
  it('is theme system, scale 1, navigation fusion and empty lists and map', () => {
    expect(DEFAULT_SETTINGS).toEqual({
      theme: 'system',
      uiScale: 1,
      navigation: 'fusion',
      paletteRecent: [],
      hiddenChecks: [],
      moreExpanded: {},
    })
  })
})

describe('loadSettings', () => {
  it('U-71 malformed JSON returns DEFAULT_SETTINGS', () => {
    localStorage.setItem(KEY, '{"theme": "dark",')
    expect(loadSettings(localStorage)).toEqual(DEFAULT_SETTINGS)
  })

  it('U-71 {"theme": "dark", "uiScale": 7} returns theme dark with scale 1', () => {
    localStorage.setItem(KEY, '{"theme": "dark", "uiScale": 7}')
    expect(loadSettings(localStorage)).toEqual({ ...DEFAULT_SETTINGS, theme: 'dark', uiScale: 1 })
  })

  it('returns DEFAULT_SETTINGS when nothing is stored', () => {
    expect(loadSettings(localStorage)).toEqual(DEFAULT_SETTINGS)
  })

  it('returns DEFAULT_SETTINGS without storage', () => {
    expect(loadSettings(null)).toEqual(DEFAULT_SETTINGS)
  })

  it('returns DEFAULT_SETTINGS when the storage throws', () => {
    expect(loadSettings(blockedStorage())).toEqual(DEFAULT_SETTINGS)
  })

  it('returns DEFAULT_SETTINGS for JSON that is not an object', () => {
    localStorage.setItem(KEY, '[1, 2]')
    expect(loadSettings(localStorage)).toEqual(DEFAULT_SETTINGS)
    localStorage.setItem(KEY, 'null')
    expect(loadSettings(localStorage)).toEqual(DEFAULT_SETTINGS)
  })

  it('keeps every valid member', () => {
    const stored: Settings = {
      theme: 'light',
      uiScale: 1.25,
      navigation: 'freecad',
      paletteRecent: ['feature.extrude', 'file.save'],
      hiddenChecks: ['thin_walls'],
      moreExpanded: { extrude: true, fillet: false },
    }
    localStorage.setItem(KEY, JSON.stringify(stored))
    expect(loadSettings(localStorage)).toEqual(stored)
  })

  it('replaces each invalid member with its default and keeps the valid ones', () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({
        theme: 'blue',
        uiScale: 2,
        navigation: 'blender',
        paletteRecent: 'file.save',
        hiddenChecks: ['islands'],
        moreExpanded: 3,
      }),
    )
    expect(loadSettings(localStorage)).toEqual({
      ...DEFAULT_SETTINGS,
      uiScale: 2,
      hiddenChecks: ['islands'],
    })
  })

  it.each([1, 1.25, 1.5, 2, 3])('accepts the UI scale %s', (uiScale) => {
    localStorage.setItem(KEY, JSON.stringify({ uiScale }))
    expect(loadSettings(localStorage).uiScale).toBe(uiScale)
  })

  it('keeps at most 20 recent palette ids', () => {
    const ids = Array.from({ length: 25 }, (_, index) => `command.${String(index)}`)
    localStorage.setItem(KEY, JSON.stringify({ paletteRecent: ids }))
    const loaded = loadSettings(localStorage).paletteRecent
    expect(loaded.length).toBeLessThanOrEqual(20)
    expect(ids.slice(0, loaded.length)).toEqual(loaded)
  })
})

describe('saveSettings', () => {
  it('stores the settings as JSON under partsmith.settings.v1', () => {
    const settings: Settings = { ...DEFAULT_SETTINGS, theme: 'dark' }
    saveSettings(localStorage, settings)
    const stored = localStorage.getItem(KEY)
    expect(stored).toContain('"theme":"dark"')
    expect(JSON.parse(stored ?? 'null')).toEqual(settings)
  })

  it('round-trips through loadSettings', () => {
    const settings: Settings = {
      theme: 'dark',
      uiScale: 3,
      navigation: 'freecad',
      paletteRecent: ['settings.theme.dark'],
      hiddenChecks: ['overhangs', 'islands'],
      moreExpanded: { shell: true },
    }
    saveSettings(localStorage, settings)
    expect(loadSettings(localStorage)).toEqual(settings)
  })

  it('does nothing without storage', () => {
    expect(() => {
      saveSettings(null, DEFAULT_SETTINGS)
    }).not.toThrow()
  })

  it('does not throw when the storage throws', () => {
    expect(() => {
      saveSettings(blockedStorage(), DEFAULT_SETTINGS)
    }).not.toThrow()
  })
})
