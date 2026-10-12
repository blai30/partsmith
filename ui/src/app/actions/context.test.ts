import { describe, expect, it } from 'vitest'
import { startWithDocument } from '../../test/fakes/app'
import { CUBE_DOCUMENT, CUBE_SKETCH_SOLUTION, CUBE_TOP_FACE } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { startFeatureCommand } from './command'
import { setContext } from './context'
import { select } from './selection'
import { enterSketch } from './sketch'

async function cube() {
  return startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
}

describe('setContext (3.3)', () => {
  it('Model to Print cancels an active feature command first and keeps the selection', async () => {
    const fake = await cube()
    await startFeatureCommand(fake.app, 'box')
    await settle()
    const face = elementItem(CUBE_TOP_FACE, 'Cube')
    select(fake.app, [face], 'replace')
    fake.transport.clearRequests()
    await setContext(fake.app, 'print')
    await settle()
    expect(fake.transport.commandTypes()).toContain('cancel_transaction')
    const state = fake.app.store.getState()
    expect(state.context).toBe('print')
    expect(state.command).toBeNull()
    expect(state.selection.map((item) => item.key)).toEqual([face.key])
  })

  it('Print to Model switches back', async () => {
    const fake = await cube()
    await setContext(fake.app, 'print')
    await setContext(fake.app, 'model')
    expect(fake.app.store.getState().context).toBe('model')
  })

  it('choosing Sketch outside sketch mode without a plane selected starts plane picking', async () => {
    const fake = await cube()
    await setContext(fake.app, 'sketch')
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch?.sketchId).toBeNull()
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
  })

  it('choosing Model in sketch mode finishes the sketch, then switches', async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    expect(fake.app.store.getState().context).toBe('sketch')
    await setContext(fake.app, 'model')
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('model')
    expect(state.sketch).toBeNull()
  })

  it('choosing Print in sketch mode finishes the sketch and switches to Print', async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    await setContext(fake.app, 'print')
    await settle()
    expect(fake.app.store.getState().context).toBe('print')
    expect(fake.app.store.getState().sketch).toBeNull()
  })
})
