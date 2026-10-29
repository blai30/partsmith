import { describe, expect, it } from 'vitest'
import type { Command, ElementDescriptor } from '../../generated/protocol'
import { escapeStep } from '../../layout/ladders'
import { registerMissingCommands } from '../../layout/test-app.testing'
import { MESSAGES } from '../../messages'
import {
  activeCommand,
  currentDocument,
  type FakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import { deferred, type Deferred } from '../../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_TOP_FRONT_EDGE,
  PARAMETRIC_DOCUMENT,
  featuresOf,
} from '../../test/fakes/documents'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { capturePick, confirmCommand, editFeature, startFeatureCommand } from './command'
import { redo, undo } from './document'
import { updateParameter } from './parameters'
import { select } from './selection'
import { startNewSketch } from './sketch'

// What arrives while OK runs (ui.md 3.7 confirming, 3.20): it waits for the confirm and then acts on
// what the confirm left, the document once the commit has landed, or the command still open when
// the confirm failed. The fake core answers in request order, as the worker does, and the test
// holds one answer to act in the window it leaves open.

const SECOND_EDGE: ElementDescriptor = {
  ...CUBE_TOP_FRONT_EDGE,
  name: 'edge(f-ext1/end-cap|f-ext1/side-1)',
  reference: {
    q: 'edge(f-ext1/end-cap|f-ext1/side-1)',
    fp: 'line|n=0,1,0|c=10,0,20|l=20|adj=2,2',
  },
}

// The transaction and edit commands, in order; queries are left out.
const RELEVANT_TYPES: ReadonlySet<Command['type']> = new Set<Command['type']>([
  'begin_transaction',
  'apply_edits',
  'commit_transaction',
  'cancel_transaction',
  'undo',
  'redo',
])

function relevantTypes(fake: FakeApp): Command['type'][] {
  return fake.transport.commandTypes().filter((type) => RELEVANT_TYPES.has(type))
}

/** Holds the core's answers to one command type until the test resolves the returned promise. */
function hold(fake: FakeApp, type: 'commit_transaction' | 'apply_edits'): Deferred<undefined> {
  const held = deferred<undefined>()
  fake.transport.handle(type, async (command, core, buffers) => {
    await held.promise
    return core.reply(command, buffers)
  })
  return held
}

/** A Fillet on the cube with its top front edge picked: one history step, preview shown. */
async function filletWithEdge(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  await startFeatureCommand(fake.app, 'fillet')
  await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
  await settle()
  return fake
}

/** A Fillet on the cube started with its edge selected: complete, with an empty history. */
async function preselectedFillet(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  select(fake.app, [elementItem(CUBE_TOP_FRONT_EDGE, 'Cube')], 'replace')
  await startFeatureCommand(fake.app, 'fillet')
  await settle()
  return fake
}

/** Presses OK with the commit held: the returned function answers the commit and waits for OK. */
async function okWithCommitHeld(fake: FakeApp): Promise<() => Promise<void>> {
  fake.transport.clearRequests()
  const commit = hold(fake, 'commit_transaction')
  const confirmed = confirmCommand(fake.app)
  await settle()
  expect(relevantTypes(fake)).toEqual(['commit_transaction'])
  return async () => {
    commit.resolve(undefined)
    await confirmed
    await settle()
  }
}

function filletCount(fake: FakeApp): number {
  return featuresOf(currentDocument(fake.app).recipe).filter((record) => record.type === 'fillet')
    .length
}

describe('Mod+Z and Mod+Shift+Z while OK runs (3.7, 3.20)', () => {
  it('E-78 Mod+Z right after OK undoes the edit and its parameter change with one core undo', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    await editFeature(fake.app, 'f-ext1')
    await updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    await settle()
    const land = await okWithCommitHeld(fake)
    const undone = undo(fake.app)
    await land()
    await undone
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction', 'undo'])
    expect(currentDocument(fake.app).recipe.parameters?.[0]?.expr).toBe('20 mm')
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('Mod+Z right after OK undoes the committed Fillet, not its last pick', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    const undone = undo(fake.app)
    await land()
    await undone
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction', 'undo'])
    expect(filletCount(fake)).toBe(0)
    expect(currentDocument(fake.app).redoLabel).toBe('Fillet')
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('Mod+Z while OK applies the last pick commits what OK was pressed on, then undoes it', async () => {
    const fake = await filletWithEdge()
    fake.transport.clearRequests()
    const preview = hold(fake, 'apply_edits')
    await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
    const confirmed = confirmCommand(fake.app)
    const undone = undo(fake.app)
    preview.resolve(undefined)
    await confirmed
    await undone
    await settle()
    expect(relevantTypes(fake)).toEqual(['apply_edits', 'commit_transaction', 'undo'])
    expect(currentDocument(fake.app).redoLabel).toBe('Fillet')
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('Mod+Shift+Z while OK runs answers for the document, not with finish_command_first', async () => {
    const fake = await filletWithEdge()
    registerMissingCommands(fake.app)
    const land = await okWithCommitHeld(fake)
    expect(fake.app.commands.get('edit.redo')?.availability(fake.app)).toEqual({
      state: 'disabled',
      reason: MESSAGES.nothing_to_redo(),
      code: 'nothing_to_redo',
    })
    await land()
  })

  it('Mod+Shift+Z right after OK redoes in the document', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    const redone = redo(fake.app)
    await land()
    await redone
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction', 'redo'])
    expect(filletCount(fake)).toBe(1)
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('Mod+Z while a failing OK runs steps back in the command it leaves open', async () => {
    const fake = await filletWithEdge()
    fake.transport.handle('apply_edits', () => ({
      error: { code: 'edit_rejected', message: 'The fillet does not fit.' },
    }))
    await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
    fake.transport.clearRequests()
    const confirmed = confirmCommand(fake.app)
    const undone = undo(fake.app)
    await confirmed
    await undone
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
    expect(activeCommand(fake.app).params['edges']).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(activeCommand(fake.app).history).toHaveLength(1)
  })
})

describe('Esc, Cancel and the close button while OK runs (3.7)', () => {
  it('cancel nothing once the commit has landed', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    expect(escapeStep(fake.app)).toBe(true)
    await land()
    expect(relevantTypes(fake)).toEqual(['commit_transaction'])
    expect(filletCount(fake)).toBe(1)
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('let OK commit while it applies the last pick', async () => {
    const fake = await filletWithEdge()
    fake.transport.clearRequests()
    const preview = hold(fake, 'apply_edits')
    await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
    const confirmed = confirmCommand(fake.app)
    expect(escapeStep(fake.app)).toBe(true)
    preview.resolve(undefined)
    await confirmed
    await settle()
    expect(relevantTypes(fake)).toEqual(['apply_edits', 'commit_transaction'])
    expect(filletCount(fake)).toBe(1)
  })
})

describe('commands, sketches and parameter changes while OK runs (3.7)', () => {
  it('another feature command opens after the commit, without command_canceled', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    const started = startFeatureCommand(fake.app, 'chamfer')
    await land()
    await started
    await settle()
    expect(relevantTypes(fake).slice(0, 2)).toEqual(['commit_transaction', 'begin_transaction'])
    expect(relevantTypes(fake)).not.toContain('cancel_transaction')
    expect(fake.app.store.getState().notifications).toEqual([])
    expect(activeCommand(fake.app).featureType).toBe('chamfer')
    expect(filletCount(fake)).toBe(1)
  })

  it('the same command started again (F after Enter, Repeat) opens a new one after the commit', async () => {
    const fake = await preselectedFillet()
    const confirmed = activeCommand(fake.app).featureId
    const land = await okWithCommitHeld(fake)
    const started = startFeatureCommand(fake.app, 'fillet')
    await land()
    await started
    await settle()
    expect(relevantTypes(fake).slice(0, 2)).toEqual(['commit_transaction', 'begin_transaction'])
    expect(activeCommand(fake.app)).toMatchObject({ featureType: 'fillet', editing: false })
    expect(activeCommand(fake.app).featureId).not.toBe(confirmed)
  })

  it('editing the feature OK is committing opens its edit after the commit', async () => {
    const fake = await preselectedFillet()
    const confirmed = activeCommand(fake.app).featureId
    const land = await okWithCommitHeld(fake)
    const edited = editFeature(fake.app, confirmed)
    await land()
    await edited
    await settle()
    expect(fake.transport.commandsOfType('begin_transaction')).toEqual([
      expect.objectContaining({ label: 'Edit Fillet 1' }),
    ])
    expect(activeCommand(fake.app)).toMatchObject({ featureId: confirmed, editing: true })
  })

  it('a sketch started meanwhile opens plane picking after the commit, canceling nothing', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    const started = startNewSketch(fake.app)
    await land()
    await started
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction'])
    expect(fake.app.store.getState().notifications).toEqual([])
    expect(fake.app.store.getState().sketch).toMatchObject({ sketchId: null })
    expect(filletCount(fake)).toBe(1)
  })

  it('a parameter change is a labeled change of its own once the commit has landed', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    await editFeature(fake.app, 'f-ext1')
    await settle()
    const land = await okWithCommitHeld(fake)
    const changed = updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    await land()
    expect(await changed).toBe(true)
    const [change] = fake.transport.commandsOfType('apply_edits')
    expect(change?.label).toBe('Change size')
    expect(currentDocument(fake.app).undoLabel).toBe('Change size')
    expect(fake.app.store.getState().notifications).toEqual([])
  })
})
