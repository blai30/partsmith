import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS } from '../settings/settings'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { createAppStore, INITIAL_STATE } from './store'

describe('INITIAL_STATE', () => {
  it('starts with the core loading and nothing loaded', () => {
    expect(INITIAL_STATE.core).toEqual({
      status: 'loading',
      engineVersion: null,
      fatal: null,
      pendingMutations: 0,
    })
    expect(INITIAL_STATE.catalog).toBeNull()
    expect(INITIAL_STATE.printers).toEqual([])
    expect(INITIAL_STATE.document).toBeNull()
  })

  it('starts in the Model context with no selection, hover, command or sketch session', () => {
    expect(INITIAL_STATE.context).toBe('model')
    expect(INITIAL_STATE.selection).toEqual([])
    expect(INITIAL_STATE.hover).toBeNull()
    expect(INITIAL_STATE.command).toBeNull()
    expect(INITIAL_STATE.sketch).toBeNull()
  })

  it('has no print report and the overhang map on', () => {
    expect(INITIAL_STATE.print).toEqual({
      report: null,
      reportRevision: null,
      running: false,
      overhangMap: true,
      focusedFinding: null,
    })
  })

  it('views in perspective from iso with faces, edges and vertices selectable and bodies not, grid on', () => {
    expect(INITIAL_STATE.view).toEqual({
      projection: 'perspective',
      named: 'iso',
      section: null,
      filter: { body: false, face: true, edge: true, vertex: true },
      sessionVisible: {},
      locked: [],
      grid: true,
      gridSpacing: null,
    })
  })

  it('has the timeline expanded and the palette, side panel and dragged inspector position unset', () => {
    expect(INITIAL_STATE.panels).toMatchObject({
      timelineCollapsed: false,
      paletteOpen: false,
      side: null,
      inspectorPosition: null,
    })
  })

  it('reads the settings from loadSettings, the defaults with nothing stored', () => {
    expect(INITIAL_STATE.settings).toEqual(DEFAULT_SETTINGS)
  })

  it('has no notifications', () => {
    expect(INITIAL_STATE.notifications).toEqual([])
  })
})

describe('createAppStore', () => {
  it('starts from INITIAL_STATE', () => {
    expect(createAppStore().getState()).toEqual(INITIAL_STATE)
  })

  it('applies overrides on top of INITIAL_STATE', () => {
    const store = createAppStore({ context: 'print', catalog: TEST_CATALOG })
    expect(store.getState().context).toBe('print')
    expect(store.getState().catalog).toBe(TEST_CATALOG)
    expect(store.getState().selection).toEqual([])
  })

  it('creates isolated stores', () => {
    const first = createAppStore()
    const second = createAppStore()
    first.setState({ context: 'sketch' })
    expect(second.getState().context).toBe('model')
  })

  it('notifies subscribers of changes', () => {
    const store = createAppStore()
    const contexts: string[] = []
    const unsubscribe = store.subscribe((state) => {
      contexts.push(state.context)
    })
    store.setState({ context: 'print' })
    unsubscribe()
    store.setState({ context: 'model' })
    expect(contexts).toEqual(['print'])
  })
})
