import { describe, expect, it } from 'vitest'
import { UI_SCALE_CLASSES } from './roles'
import { applyTheme, applyUiScale, resolveTheme, toggledTheme } from './theme'

function scaleClasses(root: HTMLElement): string[] {
  const all: readonly string[] = Object.values(UI_SCALE_CLASSES)
  return Array.from(root.classList).filter((name) => all.includes(name))
}

describe('resolveTheme', () => {
  it('U-71 system with a dark OS is dark', () => {
    expect(resolveTheme('system', true)).toBe('dark')
  })

  it('system with a light OS is light', () => {
    expect(resolveTheme('system', false)).toBe('light')
  })

  it('U-71 light stays light with a dark OS', () => {
    expect(resolveTheme('light', true)).toBe('light')
  })

  it('dark stays dark with a light OS', () => {
    expect(resolveTheme('dark', false)).toBe('dark')
  })
})

describe('applyTheme', () => {
  it('U-71 dark adds the class dark and leaves the other classes untouched', () => {
    const root = document.createElement('html')
    root.className = 'scheme-light dark:scheme-dark text-[125%]'
    applyTheme(root, 'dark')
    expect(Array.from(root.classList).toSorted()).toEqual(
      ['dark', 'dark:scheme-dark', 'scheme-light', 'text-[125%]'].toSorted(),
    )
  })

  it('U-71 light removes the class dark and leaves the other classes untouched', () => {
    const root = document.createElement('html')
    root.className = 'scheme-light dark dark:scheme-dark text-[125%]'
    applyTheme(root, 'light')
    expect(Array.from(root.classList).toSorted()).toEqual(
      ['dark:scheme-dark', 'scheme-light', 'text-[125%]'].toSorted(),
    )
  })

  it('applying the same theme twice keeps one dark class', () => {
    const root = document.createElement('html')
    applyTheme(root, 'dark')
    applyTheme(root, 'dark')
    expect(root.className.split(/\s+/).filter((name) => name === 'dark')).toHaveLength(1)
  })
})

describe('applyUiScale', () => {
  it('U-71 applyUiScale(root, 1.5) after applyUiScale(root, 2) leaves exactly one scale class, text-[150%]', () => {
    const root = document.createElement('html')
    root.className = 'scheme-light dark:scheme-dark dark'
    applyUiScale(root, 2)
    applyUiScale(root, 1.5)
    expect(scaleClasses(root)).toEqual(['text-[150%]'])
    expect(root).toHaveClass('scheme-light', 'dark:scheme-dark', 'dark')
  })

  it.each([
    [1, 'text-[100%]'],
    [1.25, 'text-[125%]'],
    [1.5, 'text-[150%]'],
    [2, 'text-[200%]'],
    [3, 'text-[300%]'],
  ] as const)('puts the class of scale %s on the root', (scale, name) => {
    const root = document.createElement('html')
    applyUiScale(root, 3)
    applyUiScale(root, scale)
    expect(scaleClasses(root)).toEqual([name])
  })
})

describe('toggledTheme', () => {
  it('U-94 toggledTheme(dark) is light', () => {
    expect(toggledTheme('dark')).toBe('light')
  })

  it('toggledTheme(light) is dark', () => {
    expect(toggledTheme('light')).toBe('dark')
  })
})
