import { describe, expect, it } from 'vitest'
import { createTestApp } from '../../test/fakes/test-app'
import {
  closeSidePanel,
  openSidePanel,
  setBrowserOpen,
  setPaletteOpen,
  showProblemsAction,
  togglePanel,
} from './panels'

describe('panel actions', () => {
  it('openSidePanel replaces the side panel and closeSidePanel closes it', () => {
    const app = createTestApp()
    openSidePanel(app, 'parameters')
    expect(app.store.getState().panels.side).toBe('parameters')
    openSidePanel(app, 'keys')
    expect(app.store.getState().panels.side).toBe('keys')
    closeSidePanel(app)
    expect(app.store.getState().panels.side).toBeNull()
  })

  it('setBrowserOpen and setPaletteOpen set their switch and keep the other panels', () => {
    const app = createTestApp()
    openSidePanel(app, 'problems')
    setBrowserOpen(app, false)
    setPaletteOpen(app, true)
    const panels = app.store.getState().panels
    expect(panels.browserOpen).toBe(false)
    expect(panels.paletteOpen).toBe(true)
    expect(panels.side).toBe('problems')
  })

  it('togglePanel flips only the named switch', () => {
    const app = createTestApp()
    const before = app.store.getState().panels
    togglePanel(app, 'timelineCollapsed')
    expect(app.store.getState().panels).toEqual({
      ...before,
      timelineCollapsed: !before.timelineCollapsed,
    })
  })

  it('showProblemsAction opens the problems panel', () => {
    const app = createTestApp()
    const action = showProblemsAction(app)
    expect(action.label).toBe('Show problems')
    action.run()
    expect(app.store.getState().panels.side).toBe('problems')
  })
})
