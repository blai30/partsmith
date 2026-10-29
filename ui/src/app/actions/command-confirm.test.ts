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
  CUBE_TOP_FACE,
  CUBE_TOP_FRONT_EDGE,
  PARAMETRIC_DOCUMENT,
  featuresOf,
} from '../../test/fakes/documents'
import { createFakeFileHandle } from '../../test/fakes/files'
import { elementItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import type { PickResult } from '../../viewport/picking'
import { capturePick, confirmCommand, editFeature, startFeatureCommand } from './command'
import { redo, undo } from './document'
import { updateParameter } from './parameters'
import { pickAt, select } from './selection'
import { startNewSketch } from './sketch'

// What arrives while OK runs (ui.md 3.7 confirming, 3.20): it waits for the confirm and then acts on
// what the confirm left, the document once the commit has landed, or the command still open when
// the confirm failed, while a click made before OK lands in the command first. The fake core
// answers in request order, as the worker does, and the test holds one answer to act in the window
// it leaves open.

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

/** cube.ptsm opened from a file, its commands registered, with a Fillet on one edge. */
async function savedFilletWithEdge(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    handle: createFakeFileHandle('cube.ptsm'),
  })
  registerMissingCommands(fake.app)
  await startFeatureCommand(fake.app, 'fillet')
  await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
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

/** Makes the core refuse every commit, so OK leaves the command open. */
function refuseCommits(fake: FakeApp): void {
  fake.transport.handle('commit_transaction', () => ({
    error: { code: 'internal_error', message: 'The edit could not be recorded.' },
  }))
}

function filletCount(fake: FakeApp): number {
  return featuresOf(currentDocument(fake.app).recipe).filter((record) => record.type === 'fillet')
    .length
}

/** The edges of the document's Fillet. */
function filletEdges(fake: FakeApp): unknown {
  return featuresOf(currentDocument(fake.app).recipe).find((record) => record.type === 'fillet')?.[
    'edges'
  ]
}

/** A click on the model, which pick_reference turns into `element`. */
function clickOn(fake: FakeApp, element: ElementDescriptor, index: number): Promise<void> {
  fake.transport.handle('pick_reference', () => ({ result: { type: 'pick_reference', element } }))
  const sample: PickResult = {
    kind: 'mesh',
    body: element.body,
    meshId: 1,
    element: element.kind,
    index,
    key: `mesh:1:${element.kind}:${String(index)}`,
    world: element.centroid,
  }
  return pickAt(fake.app, sample, { toggle: false })
}

function selectionKeys(fake: FakeApp): string[] {
  return fake.app.store.getState().selection.map((item) => item.key)
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

describe('commands a feature command disables, pressed while OK runs (3.7)', () => {
  it('are enabled, since they wait for OK', async () => {
    const fake = await savedFilletWithEdge()
    const land = await okWithCommitHeld(fake)
    for (const id of ['file.save', 'file.save_as', 'file.export', 'document.units.in']) {
      expect(fake.app.commands.get(id)?.availability(fake.app)).toEqual({ state: 'enabled' })
    }
    await land()
  })

  it('Mod+S right after OK saves the document with the confirmed feature', async () => {
    const fake = await savedFilletWithEdge()
    const land = await okWithCommitHeld(fake)
    const saved = fake.app.commands.run(fake.app, 'file.save', 'shortcut')
    await land()
    await saved
    await settle()
    const types = fake.transport.commandTypes()
    expect(
      types.filter((type) => type === 'commit_transaction' || type === 'save_document'),
    ).toEqual(['commit_transaction', 'save_document'])
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('a units choice right after OK changes the units of the document it leaves', async () => {
    const fake = await savedFilletWithEdge()
    const land = await okWithCommitHeld(fake)
    const changed = fake.app.commands.run(fake.app, 'document.units.in', 'palette')
    await land()
    await changed
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction'])
    expect(fake.transport.commandTypes()).toContain('set_document_units')
    expect(currentDocument(fake.app).recipe.units).toBe('in')
  })

  it('Mod+S while a failing OK runs is refused with finish_command_first', async () => {
    const fake = await savedFilletWithEdge()
    refuseCommits(fake)
    fake.transport.clearRequests()
    const confirmed = confirmCommand(fake.app)
    const saved = fake.app.commands.run(fake.app, 'file.save', 'shortcut')
    await confirmed
    await saved
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('save_document')
    expect(fake.app.store.getState().notifications).toContainEqual(
      expect.objectContaining({
        message: MESSAGES.finish_command_first('Fillet'),
        code: 'finish_command_first',
      }),
    )
  })
})

describe('the palette details of Undo and Redo while OK runs (3.20)', () => {
  it('Undo names the feature OK is committing, which it will undo', async () => {
    const fake = await filletWithEdge()
    registerMissingCommands(fake.app)
    const land = await okWithCommitHeld(fake)
    expect(fake.app.commands.get('edit.undo')?.detail?.(fake.app)).toBe('Undo Fillet')
    await land()
  })

  it('Undo names the edit OK is committing', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    registerMissingCommands(fake.app)
    await editFeature(fake.app, 'f-ext1')
    await updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    await settle()
    const land = await okWithCommitHeld(fake)
    expect(fake.app.commands.get('edit.undo')?.detail?.(fake.app)).toBe('Undo Edit Extrude 1')
    await land()
  })

  it('Redo shows no step of the command, which it no longer redoes', async () => {
    const fake = await filletWithEdge()
    registerMissingCommands(fake.app)
    await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
    await undo(fake.app)
    await settle()
    const land = await okWithCommitHeld(fake)
    expect(fake.app.commands.get('edit.redo')?.detail?.(fake.app)).toBeNull()
    await land()
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

describe('clicks on the model around OK (3.7)', () => {
  it('an edge clicked right before OK is part of the confirmed feature, however late the core answers', async () => {
    const fake = await filletWithEdge()
    fake.transport.clearRequests()
    fake.transport.pause()
    const click = clickOn(fake, SECOND_EDGE, 1)
    const confirmed = confirmCommand(fake.app)
    fake.transport.resume()
    await Promise.all([click, confirmed])
    await settle()
    expect(relevantTypes(fake)).toEqual(['apply_edits', 'commit_transaction'])
    expect(filletEdges(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference, SECOND_EDGE.reference])
    expect(selectionKeys(fake)).toEqual([])
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('an edge clicked while OK runs is selected once the commit has landed', async () => {
    const fake = await filletWithEdge()
    const land = await okWithCommitHeld(fake)
    const click = clickOn(fake, SECOND_EDGE, 1)
    await land()
    await click
    expect(filletEdges(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(selectionKeys(fake)).toEqual([`element:b-cube:${SECOND_EDGE.name}`])
  })

  it('a face clicked while OK applies the last pick is selected once the commit has landed', async () => {
    const fake = await filletWithEdge()
    fake.transport.clearRequests()
    const preview = hold(fake, 'apply_edits')
    await capturePick(fake.app, elementItem(SECOND_EDGE, 'Cube'))
    const confirmed = confirmCommand(fake.app)
    const click = clickOn(fake, CUBE_TOP_FACE, 0)
    preview.resolve(undefined)
    await Promise.all([confirmed, click])
    await settle()
    expect(filletEdges(fake)).toEqual([CUBE_TOP_FRONT_EDGE.reference, SECOND_EDGE.reference])
    expect(selectionKeys(fake)).toEqual([`element:b-cube:${CUBE_TOP_FACE.name}`])
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

  it('the same command started while a failing OK runs keeps the command it leaves open', async () => {
    const fake = await preselectedFillet()
    const kept = activeCommand(fake.app).featureId
    refuseCommits(fake)
    fake.transport.clearRequests()
    const confirmed = confirmCommand(fake.app)
    const started = startFeatureCommand(fake.app, 'fillet')
    await confirmed
    await started
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction'])
    expect(activeCommand(fake.app).featureId).toBe(kept)
    expect(activeCommand(fake.app).params['edges']).toEqual([CUBE_TOP_FRONT_EDGE.reference])
  })

  it('editing the feature again while a failing OK runs keeps the edit it leaves open', async () => {
    const fake = await startWithDocument('parametric.ptsm', PARAMETRIC_DOCUMENT)
    await editFeature(fake.app, 'f-ext1')
    await updateParameter(fake.app, 'p-size', { expression: '30 mm' })
    await settle()
    refuseCommits(fake)
    fake.transport.clearRequests()
    const confirmed = confirmCommand(fake.app)
    const edited = editFeature(fake.app, 'f-ext1')
    await confirmed
    await edited
    await settle()
    expect(relevantTypes(fake)).toEqual(['commit_transaction'])
    expect(activeCommand(fake.app)).toMatchObject({ featureId: 'f-ext1', editing: true })
    expect(currentDocument(fake.app).recipe.parameters?.[0]?.expr).toBe('30 mm')
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
