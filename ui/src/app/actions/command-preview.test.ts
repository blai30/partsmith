import { afterEach, describe, expect, it, vi } from 'vitest'
import type { ElementDescriptor, Edit } from '../../generated/protocol'
import {
  CUBE_DOCUMENT,
  CUBE_FEATURES_DOCUMENT,
  CUBE_REGION,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FRONT_EDGE,
  featuresOf,
  RECT_SKETCH_DOCUMENT,
  RECT_SKETCH_SOLUTION,
  withFeatureChanges,
} from '../../test/fakes/documents'
import { createTestApp, startWithDocument, type FakeApp } from '../../test/fakes/app'
import { createFakeCoreApi } from '../../test/fakes/core-api'
import { stateWithDocument } from '../../test/fakes/state'
import { createAppStore, type ActiveCommand } from '../../state/store'
import { editsOf } from '../../test/fakes/protocol'
import { elementItem, originItem, regionItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { MESSAGES } from '../../messages'
import {
  cancelCommand,
  capturePick,
  confirmCommand,
  editFeature,
  setCommandField,
  setFieldStatus,
  startFeatureCommand,
} from './command'
import { select } from './selection'

// The preview cycle and selection capture of ui.md 3.7, beyond the acceptance tests in command.test.ts.

function appliedEdits(fake: FakeApp): readonly (readonly Edit[])[] {
  return fake.transport.commandsOfType('apply_edits').map((command) => editsOf(command))
}

async function cubeExtrude(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
    core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
  })
  select(fake.app, [regionItem('f-sk1', CUBE_REGION)], 'replace')
  await startFeatureCommand(fake.app, 'extrude')
  await settle()
  fake.transport.clearRequests()
  return fake
}

describe('op and the body a command creates', () => {
  it('starts joining the last body with geometry', async () => {
    const fake = await cubeExtrude()
    expect(fake.app.store.getState().command?.params).toMatchObject({ op: 'join', body: 'b-cube' })
  })

  it('switching to new adds a fresh body, and switching back removes it and targets the last body', async () => {
    const fake = await cubeExtrude()
    setCommandField(fake.app, 'op', 'new')
    await settle()
    const [added] = appliedEdits(fake)
    expect(added?.map((edit) => edit.type)).toEqual(['add_body', 'update_feature'])
    const newBody = fake.app.store.getState().command?.newBody
    expect(newBody?.name).toBe('Body 1')
    expect(fake.app.store.getState().command?.params['body']).toBe(newBody?.id)

    setCommandField(fake.app, 'op', 'join')
    await settle()
    const removed = appliedEdits(fake)[1] ?? []
    expect(removed.map((edit) => edit.type)).toEqual(['update_feature', 'remove_body'])
    expect(removed[1]).toEqual({ type: 'remove_body', id: newBody?.id })
    expect(fake.app.store.getState().command).toMatchObject({ newBody: null })
    expect(fake.app.store.getState().command?.params['body']).toBe('b-cube')
  })
})

describe('field status', () => {
  it('a pending field holds the preview back until its result arrives', async () => {
    const fake = await cubeExtrude()
    setFieldStatus(fake.app, 'extent.expr', { error: null, pending: true })
    setCommandField(fake.app, 'extent.expr', '2 * 15 mm')
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
    expect(fake.app.store.getState().command?.preview).toBe('none')
    setFieldStatus(fake.app, 'extent.expr', { error: null, pending: false })
    await settle()
    expect(appliedEdits(fake)).toHaveLength(1)
    expect(fake.app.store.getState().command?.preview).toBe('shown')
  })

  it('a preview that lands after its field became invalid stays hidden', async () => {
    const fake = await cubeExtrude()
    fake.transport.pause()
    setCommandField(fake.app, 'extent.expr', '30 mm')
    expect(fake.app.store.getState().command?.preview).toBe('pending')
    setFieldStatus(fake.app, 'extent.expr', { error: 'Distance needs a value.', pending: false })
    expect(fake.app.store.getState().command?.preview).toBe('none')
    fake.transport.resume()
    await settle()
    expect(fake.app.store.getState().command?.preview).toBe('none')
  })

  it('an invalid field is shown and keeps OK from committing', async () => {
    const fake = await cubeExtrude()
    setFieldStatus(fake.app, 'extent.expr', { error: 'Distance needs a value.', pending: false })
    expect(fake.app.store.getState().command?.fieldErrors).toEqual({
      'extent.expr': 'Distance needs a value.',
    })
    await confirmCommand(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
    expect(fake.app.store.getState().command).not.toBeNull()
  })
})

describe('confirmCommand', () => {
  it('does nothing while the params are incomplete', async () => {
    const fake = await startWithDocument('rect-sketch.ptsm', RECT_SKETCH_DOCUMENT, {
      core: { solutions: { 'f-sk1': RECT_SKETCH_SOLUTION } },
    })
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    await confirmCommand(fake.app)
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
  })

  it('hides the sketch the committed profile uses, for the session', async () => {
    const fake = await cubeExtrude()
    await confirmCommand(fake.app)
    await settle()
    expect(fake.app.store.getState().view.sessionVisible['feature:f-sk1']).toBe(false)
  })
})

async function filletOnCube(): Promise<FakeApp> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  await startFeatureCommand(fake.app, 'fillet')
  await settle()
  return fake
}

describe('capturePick', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('refuses an item the active field does not take, with the hint under the field for 3 s', async () => {
    const fake = await filletOnCube()
    vi.useFakeTimers()
    await capturePick(fake.app, originItem('XY'))
    expect(fake.app.store.getState().pickHint).toEqual({
      message: MESSAGES.pick_kind('edges'),
      field: 'edges',
    })
    expect(fake.app.store.getState().notifications).toEqual([])
    expect(fake.app.store.getState().command?.params['edges']).toBeUndefined()
    vi.advanceTimersByTime(2_999)
    expect(fake.app.store.getState().pickHint).not.toBeNull()
    vi.advanceTimersByTime(1)
    expect(fake.app.store.getState().pickHint).toBeNull()
  })

  it("refuses an edge of another body than the field's body, with the hint under the field", async () => {
    const fake = await filletOnCube()
    const pegEdge: ElementDescriptor = {
      ...CUBE_TOP_FRONT_EDGE,
      body: 'b-peg',
      name: 'edge(f-cyl1/top|f-cyl1/side)',
      reference: { q: 'edge(f-cyl1/top|f-cyl1/side)', fp: 'circle|r=4' },
    }
    // The first edge fills Body (2.12); an edge of another body is then refused.
    await capturePick(fake.app, elementItem(CUBE_TOP_FRONT_EDGE, 'Cube'))
    await capturePick(fake.app, elementItem(pegEdge, 'Peg'))
    expect(fake.app.store.getState().pickHint).toEqual({
      message: MESSAGES.pick_same_body('edges', 'Cube'),
      field: 'edges',
    })
  })

  it('a newer refusal replaces the hint and gets its own 3 s', async () => {
    const fake = await filletOnCube()
    vi.useFakeTimers()
    await capturePick(fake.app, originItem('XY'))
    vi.advanceTimersByTime(2_000)
    await capturePick(fake.app, originItem('XY'))
    vi.advanceTimersByTime(1_000)
    expect(fake.app.store.getState().pickHint?.field).toBe('edges')
    vi.advanceTimersByTime(2_000)
    expect(fake.app.store.getState().pickHint).toBeNull()
  })

  it('canceling the command removes its hint', async () => {
    const fake = await filletOnCube()
    await capturePick(fake.app, originItem('XY'))
    await cancelCommand(fake.app)
    expect(fake.app.store.getState().pickHint).toBeNull()
  })
})

describe('editFeature', () => {
  it('keeps the record name in every update', async () => {
    const fake = await startWithDocument(
      'cube.ptsm',
      withFeatureChanges(CUBE_DOCUMENT, 'f-ext1', { name: 'Base' }),
    )
    await editFeature(fake.app, 'f-ext1')
    await settle()
    expect(fake.app.store.getState().command?.params).not.toHaveProperty('name')
    setCommandField(fake.app, 'extent.expr', '25 mm')
    await settle()
    const update = appliedEdits(fake).at(-1)?.[0]
    expect(update?.type === 'update_feature' ? update.feature : null).toMatchObject({
      id: 'f-ext1',
      name: 'Base',
      extent: { kind: 'distance', expr: '25 mm' },
    })
  })

  it('applies nothing until a field changes, since the record is already the focused feature', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await editFeature(fake.app, 'f-ext1')
    await settle()
    expect(fake.transport.commandTypes()).not.toContain('apply_edits')
  })
})

describe('a command the store holds without a session', () => {
  it('is driven from the store: a field change previews the record from its params', async () => {
    const core = createFakeCoreApi()
    const edges = featuresOf(CUBE_FEATURES_DOCUMENT).find((feature) => feature.id === 'f-fil1')?.[
      'edges'
    ]
    const command: ActiveCommand = {
      featureType: 'fillet',
      featureId: 'f-fil1',
      editing: true,
      title: 'Fillet 1',
      params: { body: 'b-cube', edges, radius: '2 mm' },
      activeField: null,
      fieldErrors: {},
      applied: true,
      newBody: null,
      helper: null,
      changedPaths: [],
      history: [],
      future: [],
      preview: 'shown',
    }
    const app = createTestApp({
      core,
      store: createAppStore(stateWithDocument(CUBE_FEATURES_DOCUMENT, { command })),
    })
    setCommandField(app, 'radius', '3 mm')
    await settle()
    const [call] = core.callsTo('applyEdits')
    expect(call?.[1]).toBeNull()
    expect(call?.[2]).toEqual([
      {
        type: 'update_feature',
        feature: { id: 'f-fil1', type: 'fillet', body: 'b-cube', edges, radius: '3 mm' },
      },
    ])
  })
})
