import { describe, expect, it } from 'vitest'
import type { ActiveCommand } from '../state/store'
import { MAC_WEB } from '../test/fakes/app'
import { settle } from '../test/fakes/settle'
import { bodyItem, cubeDocumentState, originItem, readyState } from './fixtures.testing'
import { activeFieldAccepts, pickRow, togglesSelection } from './pick-row'
import { createModelApp } from './test-app.testing'

const DATUM_COMMAND: ActiveCommand = {
  featureType: 'datum_plane',
  featureId: 'f-dp9',
  editing: false,
  title: 'Datum plane',
  params: { offset: '10 mm' },
  activeField: 'base',
  fieldErrors: {},
  applied: false,
  newBody: null,
  preview: 'none',
}

describe('activeFieldAccepts', () => {
  it('is null without a command or without an active field', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    expect(activeFieldAccepts(app.store.getState())).toBeNull()
    app.store.setState({ command: { ...DATUM_COMMAND, activeField: null } })
    expect(activeFieldAccepts(app.store.getState())).toBeNull()
  })

  it('is what the active selection field accepts', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    expect(activeFieldAccepts(app.store.getState())).toBe('plane')
  })
})

describe('pickRow', () => {
  it('selects the item, replacing or toggling the selection', () => {
    const { app } = createModelApp({ state: readyState(cubeDocumentState()) })
    pickRow(app, originItem('XY'), false)
    pickRow(app, bodyItem('b-cube', 'Cube'), true)
    expect(app.store.getState().selection.map((item) => item.key)).toEqual([
      'origin:XY',
      'body:b-cube',
    ])
    pickRow(app, originItem('XZ'), false)
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['origin:XZ'])
  })

  it('fills the active field with an item it accepts instead of selecting it', async () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    pickRow(app, originItem('YZ'), false)
    await settle()
    expect(app.store.getState().command?.params.base).toEqual({ kind: 'origin', plane: 'YZ' })
    expect(app.store.getState().selection).toEqual([])
  })

  it('selects an item the active field does not accept', () => {
    const { app } = createModelApp({
      state: readyState(cubeDocumentState(), { command: DATUM_COMMAND }),
    })
    pickRow(app, bodyItem('b-cube', 'Cube'), false)
    expect(app.store.getState().command?.params.base).toBeUndefined()
    expect(app.store.getState().selection.map((item) => item.key)).toEqual(['body:b-cube'])
  })
})

describe('togglesSelection', () => {
  const none = { shiftKey: false, metaKey: false, ctrlKey: false }

  it('toggles with Shift everywhere', () => {
    const { app } = createModelApp()
    expect(togglesSelection(app, { ...none, shiftKey: true })).toBe(true)
    expect(togglesSelection(app, none)).toBe(false)
  })

  it('toggles with Ctrl outside macOS and with Cmd on macOS', () => {
    const linux = createModelApp().app
    const mac = createModelApp({ platform: MAC_WEB }).app
    expect(togglesSelection(linux, { ...none, ctrlKey: true })).toBe(true)
    expect(togglesSelection(linux, { ...none, metaKey: true })).toBe(false)
    expect(togglesSelection(mac, { ...none, metaKey: true })).toBe(true)
    expect(togglesSelection(mac, { ...none, ctrlKey: true })).toBe(false)
  })
})
