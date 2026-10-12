import { describe, expect, it } from 'vitest'
import {
  createTestApp,
  currentDocument,
  type FakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import { CUBE_DOCUMENT, bodiesOf } from '../../test/fakes/documents'
import { settle } from '../../test/fakes/settle'
import {
  renameBody,
  setBodyColor,
  setBodyMaterial,
  setBodyVisible,
  setLocked,
  setSessionVisible,
} from './bodies'

/** The one update_body edit sent, as { label, body }. */
function bodyUpdate(fake: FakeApp): { label: string | null; body: unknown } {
  const commands = fake.transport.commandsOfType('apply_edits')
  expect(commands).toHaveLength(1)
  const [command] = commands
  const [edit] = command?.edits ?? []
  expect(command?.edits).toHaveLength(1)
  return {
    label: command?.label ?? null,
    body: edit?.type === 'update_body' ? edit.body : null,
  }
}

describe('setBodyVisible (3.10)', () => {
  it('hides a body with update_body labeled "Hide {name}"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await setBodyVisible(fake.app, 'b-cube', false)
    await settle()
    expect(bodyUpdate(fake)).toEqual({
      label: 'Hide Cube',
      body: { id: 'b-cube', name: 'Cube', visible: false },
    })
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.visible).toBe(false)
  })

  it('shows a hidden body with update_body labeled "Show {name}"', async () => {
    const fake = await startWithDocument('cube.ptsm', {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', visible: false }],
    })
    await setBodyVisible(fake.app, 'b-cube', true)
    await settle()
    const { label, body } = bodyUpdate(fake)
    expect(label).toBe('Show Cube')
    expect(body).toMatchObject({ id: 'b-cube', name: 'Cube' })
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.visible ?? true).toBe(true)
  })
})

describe('setBodyColor (3.10)', () => {
  it('sets the color with update_body labeled "Change color"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await setBodyColor(fake.app, 'b-cube', '#e7000b')
    await settle()
    expect(bodyUpdate(fake)).toEqual({
      label: 'Change color',
      body: { id: 'b-cube', name: 'Cube', color: '#e7000b' },
    })
  })

  it('null removes the color (the Default swatch)', async () => {
    const fake = await startWithDocument('cube.ptsm', {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', color: '#e7000b' }],
    })
    await setBodyColor(fake.app, 'b-cube', null)
    await settle()
    const { label, body } = bodyUpdate(fake)
    expect(label).toBe('Change color')
    expect(body).toMatchObject({ id: 'b-cube', name: 'Cube' })
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.color ?? null).toBeNull()
  })
})

describe('setBodyMaterial (3.8)', () => {
  it('sets the material with update_body labeled "Change material"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await setBodyMaterial(fake.app, 'b-cube', 'PETG')
    await settle()
    expect(bodyUpdate(fake)).toEqual({
      label: 'Change material',
      body: { id: 'b-cube', name: 'Cube', material: 'PETG' },
    })
  })

  it('null clears the material', async () => {
    const fake = await startWithDocument('cube.ptsm', {
      ...CUBE_DOCUMENT,
      bodies: [{ id: 'b-cube', name: 'Cube', material: 'PLA' }],
    })
    await setBodyMaterial(fake.app, 'b-cube', null)
    await settle()
    expect(bodiesOf(currentDocument(fake.app).recipe)[0]?.material ?? null).toBeNull()
  })
})

describe('renameBody (3.10)', () => {
  it('renames with update_body labeled "Rename"', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await renameBody(fake.app, 'b-cube', 'Box')
    await settle()
    expect(bodyUpdate(fake)).toEqual({ label: 'Rename', body: { id: 'b-cube', name: 'Box' } })
    expect(currentDocument(fake.app).undoLabel).toBe('Rename')
  })
})

describe('session-only view state (3.10)', () => {
  it('setLocked adds and removes the body in view.locked without a core call', () => {
    const app = createTestApp()
    setLocked(app, 'b-cube', true)
    expect(app.store.getState().view.locked).toEqual(['b-cube'])
    setLocked(app, 'b-cube', true)
    expect(app.store.getState().view.locked).toEqual(['b-cube'])
    setLocked(app, 'b-cube', false)
    expect(app.store.getState().view.locked).toEqual([])
  })

  it('setSessionVisible records visibility by selection key', () => {
    const app = createTestApp()
    setSessionVisible(app, 'feature:f-sk1', false)
    setSessionVisible(app, 'origin:XY', true)
    expect(app.store.getState().view.sessionVisible).toEqual({
      'feature:f-sk1': false,
      'origin:XY': true,
    })
  })
})
