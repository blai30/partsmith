import { describe, expect, it } from 'vitest'
import { builtinCommands } from '../../commands/builtin'
import type { CommandDefinition } from '../../commands/registry'
import { selectSketchRadioTooltip, selectSketchToEdit } from '../../state/selectors'
import { LINUX_WEB, startWithDocument } from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_REGION,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FACE,
} from '../../test/fakes/documents'
import { bodyItem, elementItem, featureItem, regionItem } from '../../test/fakes/selection'
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

  it("U-99 choosing Model in sketch mode selects the finished sketch's only region (3.16.11)", async () => {
    const fake = await cube()
    await enterSketch(fake.app, 'f-sk1')
    await setContext(fake.app, 'model')
    await settle()
    expect(fake.app.store.getState().selection.map((item) => item.key)).toEqual([
      `region:f-sk1:${CUBE_REGION.reference.key}`,
    ])
  })

  it('choosing Print in sketch mode does not select what the sketch made (3.16.11)', async () => {
    const fake = await cube()
    // Entering the sketch clears the face it started from (3.16.1).
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube')], 'replace')
    await enterSketch(fake.app, 'f-sk1')
    await setContext(fake.app, 'print')
    await settle()
    expect(fake.app.store.getState().selection).toEqual([])
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

/** The built-in `context.sketch`: the Sketch radio's command. */
function sketchContextCommand(): CommandDefinition {
  const definition = builtinCommands(LINUX_WEB).find((command) => command.id === 'context.sketch')
  if (definition === undefined) {
    throw new Error('context.sketch is not a built-in command.')
  }
  return definition
}

describe('the Sketch radio (3.3)', () => {
  it('U-113 with the browser row of f-sk1 selected, context.sketch edits Sketch 1 and its tooltip is "Edit Sketch 1"', async () => {
    const fake = await cube()
    select(fake.app, [featureItem('f-sk1', 'Sketch 1')], 'replace')
    const command = sketchContextCommand()
    expect(command.detail?.(fake.app)).toBe('Edit Sketch 1')
    await command.run(fake.app, 'toolbar')
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch?.sketchId).toBe('f-sk1')
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
  })

  it('U-113 with a region of f-sk1 selected, context.sketch edits Sketch 1 and its tooltip is "Edit Sketch 1"', async () => {
    const fake = await cube()
    select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
    const command = sketchContextCommand()
    expect(command.detail?.(fake.app)).toBe('Edit Sketch 1')
    await command.run(fake.app, 'toolbar')
    await settle()
    expect(fake.app.store.getState().sketch?.sketchId).toBe('f-sk1')
  })

  it('U-113 with a flat face selected, context.sketch starts a new sketch on it and its tooltip is "New sketch"', async () => {
    const fake = await cube()
    select(fake.app, [elementItem(CUBE_TOP_FACE, 'Cube')], 'replace')
    const command = sketchContextCommand()
    expect(command.detail?.(fake.app)).toBe('New sketch')
    await command.run(fake.app, 'toolbar')
    await settle()
    const state = fake.app.store.getState()
    expect(fake.transport.commandTypes()).toContain('apply_edits')
    expect(state.context).toBe('sketch')
    expect(state.sketch?.sketchId).not.toBe('f-sk1')
    expect(state.sketch?.sketchId).not.toBeNull()
  })

  it('starts a new sketch for a sketch row together with a body, or with nothing selected', async () => {
    const fake = await cube()
    const state = () => fake.app.store.getState()
    expect(selectSketchToEdit(state())).toBeNull()
    expect(selectSketchRadioTooltip(state())).toBe('New sketch')
    select(fake.app, [featureItem('f-sk1', 'Sketch 1'), bodyItem('b-cube', 'Cube')], 'replace')
    expect(selectSketchToEdit(state())).toBeNull()
    select(fake.app, [featureItem('f-ext1', 'Extrude 1')], 'replace')
    expect(selectSketchToEdit(state())).toBeNull()
    expect(selectSketchRadioTooltip(state())).toBe('New sketch')
  })

  it('has no tooltip in sketch mode, where choosing Sketch changes nothing', async () => {
    const fake = await cube()
    select(fake.app, [featureItem('f-sk1', 'Sketch 1')], 'replace')
    await enterSketch(fake.app, 'f-sk1')
    await settle()
    expect(sketchContextCommand().detail?.(fake.app)).toBeNull()
    fake.transport.clearRequests()
    await setContext(fake.app, 'sketch')
    await settle()
    expect(fake.app.store.getState().sketch?.sketchId).toBe('f-sk1')
    expect(fake.transport.commandTypes()).toEqual([])
  })
})
