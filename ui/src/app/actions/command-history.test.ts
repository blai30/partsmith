import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../../generated/protocol'
import { registerMissingCommands } from '../../layout/test-app.testing'
import { MESSAGES } from '../../messages'
import { activeCommand, type FakeApp, startWithDocument } from '../../test/fakes/app'
import { CUBE_DOCUMENT, CUBE_TOP_FRONT_EDGE } from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import {
  capturePick,
  redoCommandChange,
  setCommandField,
  startFeatureCommand,
  undoCommandChange,
} from './command'
import { newDocument, redo, undo } from './document'
import { setContext } from './context'

// Undo inside a command and Restore after an implicit cancel (ui.md 3.7 command history, 3.20).

const SECOND_EDGE: ElementDescriptor = {
  ...CUBE_TOP_FRONT_EDGE,
  name: 'edge(f-ext1/end-cap|f-ext1/side-1)',
  reference: {
    q: 'edge(f-ext1/end-cap|f-ext1/side-1)',
    fp: 'line|n=0,1,0|c=10,0,20|l=20|adj=2,2',
  },
}

async function filletWithTwoEdges(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  await startFeatureCommand(fake.app, 'fillet')
  await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
  await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
  await settle()
  return fake
}

function edgesOf(fake: FakeApp): unknown {
  return activeCommand(fake.app).params['edges']
}

describe('command history (3.7)', () => {
  it('records each pick with the field it changed, and Mod+Z undoes only the last one', async () => {
    const fake = await filletWithTwoEdges()
    expect(activeCommand(fake.app).history.map((step) => step.field)).toEqual(['Edges', 'Edges'])
    await undo(fake.app)
    expect(edgesOf(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(activeCommand(fake.app).future).toHaveLength(1)
    await redo(fake.app)
    expect(edgesOf(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference, SECOND_EDGE.reference])
    expect(fake.transport.commandTypes()).not.toContain('undo')
  })

  it('does not record preselection and starting values, and a new change clears the redo steps', async () => {
    const fake = await filletWithTwoEdges()
    await undoCommandChange(fake.app)
    setCommandField(fake.app, 'radius', '2 mm', { typing: true })
    expect(activeCommand(fake.app).future).toEqual([])
    expect(activeCommand(fake.app).history.map((step) => step.field)).toEqual(['Edges', 'Radius'])
    expect(activeCommand(fake.app).changedPaths).toEqual(['edges', 'radius'])
  })

  it('records the edits since a field gained focus as one change', async () => {
    const fake = await filletWithTwoEdges()
    setCommandField(fake.app, 'radius', '2', { typing: true })
    setCommandField(fake.app, 'radius', '2.5', { typing: true })
    expect(activeCommand(fake.app).history).toHaveLength(3)
    await undoCommandChange(fake.app)
    expect(activeCommand(fake.app).params['radius']).toBe('1 mm')
  })

  it('Mod+Z with an empty history cancels the command without a core undo', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    fake.transport.clearRequests()
    await undo(fake.app)
    await settle()
    expect(fake.app.store.getState().command).toBeNull()
    expect(fake.transport.commandTypes()).toEqual(['cancel_transaction'])
  })

  it('gives Undo and Redo the details of 3.20 during a command', async () => {
    const fake = await filletWithTwoEdges()
    registerMissingCommands(fake.app)
    const detail = (id: string) => fake.app.commands.get(id)?.detail?.(fake.app) ?? null
    expect(detail('edit.undo')).toBe('Undo change to Edges')
    expect(fake.app.commands.get('edit.redo')?.availability(fake.app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.finish_command_first('Fillet'),
      code: 'finish_command_first',
    })
    await undoCommandChange(fake.app)
    expect(detail('edit.redo')).toBe('Redo change to Edges')
    expect(fake.app.commands.get('edit.redo')?.availability(fake.app)).toEqual({
      state: 'enabled',
    })
    await undoCommandChange(fake.app)
    expect(detail('edit.undo')).toBe('Cancel Fillet')
    await redoCommandChange(fake.app)
    expect(edgesOf(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
  })
})

describe('implicit cancel and Restore (3.7, U-109)', () => {
  it('U-109 starting another command cancels Fillet and offers Restore', async () => {
    const fake = await filletWithTwoEdges()
    fake.transport.clearRequests()
    await startFeatureCommand(fake.app, 'chamfer')
    await settle()
    expect(fake.transport.commandTypes()).toContain('cancel_transaction')
    const [notification] = fake.app.store.getState().notifications
    expect(notification).toMatchObject({
      severity: 'info',
      message: 'Fillet was canceled.',
      code: 'command_canceled',
    })
    expect(notification?.actions.map((action) => action.label)).toEqual(['Restore'])
  })

  it('U-109 Restore cancels Chamfer quietly and reopens Fillet with both edges and the same field', async () => {
    const fake = await filletWithTwoEdges()
    const activeField = activeCommand(fake.app).activeField
    await startFeatureCommand(fake.app, 'chamfer')
    await settle()
    const [notification] = fake.app.store.getState().notifications
    fake.transport.clearRequests()
    notification?.actions[0]?.run()
    await settle()
    expect(fake.transport.commandTypes().slice(0, 2)).toEqual([
      'cancel_transaction',
      'begin_transaction',
    ])
    const command = activeCommand(fake.app)
    expect(command.featureType).toBe('fillet')
    expect(command.params['edges']).toEqual([CUBE_TOP_FRONT_EDGE.reference, SECOND_EDGE.reference])
    expect(command.activeField).toBe(activeField)
    expect(
      fake.app.store
        .getState()
        .notifications.filter((candidate) => candidate.code === 'command_canceled'),
    ).toHaveLength(1)
  })

  it('U-109 ends a command without changes silently', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await startFeatureCommand(fake.app, 'fillet')
    await startFeatureCommand(fake.app, 'chamfer')
    await settle()
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-109 a new document ends a command with picks without the notification', async () => {
    const fake = await filletWithTwoEdges()
    await newDocument(fake.app)
    await settle()
    expect(fake.app.store.getState().command).toBeNull()
    expect(
      fake.app.store
        .getState()
        .notifications.filter((notification) => notification.code === 'command_canceled'),
    ).toEqual([])
  })

  it('a context change ends a command with picks implicitly', async () => {
    const fake = await filletWithTwoEdges()
    await setContext(fake.app, 'print')
    await settle()
    expect(
      fake.app.store.getState().notifications.map((notification) => notification.code),
    ).toContain('command_canceled')
  })
})
