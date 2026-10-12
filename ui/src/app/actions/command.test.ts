import { describe, expect, it } from 'vitest'
import type { Command, Document, Edit } from '../../generated/protocol'
import { actionLabels, currentDocument, type FakeApp, notificationWith, runAction, startWithDocument } from '../../test/fakes/app'
import {
  CUBE_DOCUMENT,
  CUBE_EXTRUDE,
  CUBE_FEATURES_DOCUMENT,
  CUBE_SKETCH_SOLUTION,
  CUBE_TOP_FRONT_EDGE,
  RECT_REGION,
  RECT_SKETCH_DOCUMENT,
  RECT_SKETCH_SOLUTION,
} from '../../test/fakes/documents'
import { elementItem, regionItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import {
  cancelCommand,
  capturePick,
  confirmCommand,
  editFeature,
  setActiveField,
  setCommandField,
  startFeatureCommand,
} from './command'
import { select } from './selection'

type ApplyEdits = Extract<Command, { type: 'apply_edits' }>
type AddFeature = Extract<Edit, { type: 'add_feature' }>
type UpdateFeature = Extract<Edit, { type: 'update_feature' }>

const ID_PATTERN = /^[fb]-[0-9a-f]{12}$/

function editsOf(command: ApplyEdits | undefined): readonly Edit[] {
  return command?.edits ?? []
}

function addedFeature(fake: FakeApp): AddFeature['feature'] | null {
  for (const command of fake.transport.commandsOfType('apply_edits')) {
    for (const edit of command.edits) {
      if (edit.type === 'add_feature') {
        return edit.feature
      }
    }
  }
  return null
}

function updatedFeatures(fake: FakeApp): UpdateFeature['feature'][] {
  return fake.transport
    .commandsOfType('apply_edits')
    .flatMap((command) => command.edits)
    .flatMap((edit) => (edit.type === 'update_feature' ? [edit.feature] : []))
}

function relevantTypes(fake: FakeApp): Command['type'][] {
  const relevant: readonly Command['type'][] = [
    'begin_transaction',
    'apply_edits',
    'set_focus',
    'commit_transaction',
    'cancel_transaction',
    'undo',
  ]
  return fake.transport.commandTypes().filter((type) => relevant.includes(type))
}

async function rectSketchWithRegionSelected(): Promise<FakeApp> {
  const fake = await startWithDocument('rect-sketch.ptsm', RECT_SKETCH_DOCUMENT, {
    core: { solutions: { 'f-sk1': RECT_SKETCH_SOLUTION } },
  })
  select(fake.app, [regionItem('f-sk1', RECT_REGION)], 'replace')
  return fake
}

describe('startFeatureCommand (3.7)', () => {
  it('U-64 with a preselected region and no bodies: begins "Extrude", applies the new body and the feature in one batch, then focuses the feature', async () => {
    const fake = await rectSketchWithRegionSelected()
    const doc = currentDocument(fake.app).documentHandle
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    expect(relevantTypes(fake)).toEqual(['begin_transaction', 'apply_edits', 'set_focus'])
    expect(fake.transport.commandsOfType('begin_transaction')).toEqual([
      { type: 'begin_transaction', doc, label: 'Extrude' },
    ])
    const [apply] = fake.transport.commandsOfType('apply_edits')
    expect(apply?.label ?? null).toBeNull()
    const edits = editsOf(apply)
    expect(edits.map((edit) => edit.type)).toEqual(['add_body', 'add_feature'])
    const [addBody, addFeature] = edits
    if (addBody?.type !== 'add_body' || addFeature?.type !== 'add_feature') {
      return
    }
    expect(addBody.index).toBe(0)
    expect(addBody.body).toMatchObject({ name: 'Body 1' })
    expect(addBody.body.id).toMatch(ID_PATTERN)
    expect(addBody.body.id.startsWith('b-')).toBe(true)
    expect(addFeature.index).toBe(1)
    const feature = addFeature.feature
    expect(feature.id).toMatch(ID_PATTERN)
    expect(feature.id.startsWith('f-')).toBe(true)
    expect(feature).toMatchObject({
      type: 'extrude',
      body: addBody.body.id,
      profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
      extent: { kind: 'distance', expr: '10 mm' },
      op: 'new',
    })
    expect('name' in feature).toBe(false)
    expect(fake.transport.commandsOfType('set_focus')).toEqual([
      { type: 'set_focus', doc, feature: feature.id },
    ])
  })

  it('U-64 sets the active command and clears the app selection', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    const state = fake.app.store.getState()
    const feature = addedFeature(fake)
    expect(state.selection).toEqual([])
    expect(state.context).toBe('model')
    expect(state.command).toMatchObject({
      featureType: 'extrude',
      featureId: feature?.id,
      editing: false,
      title: 'Extrude',
      applied: true,
      newBody: { name: 'Body 1' },
    })
    expect(state.command?.params).toMatchObject({
      profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
      op: 'new',
    })
  })

  it('U-65 with rollback at the second of three features, the first preview adds the feature at index 1 and sets no rollback', async () => {
    const recipe: Document = {
      ...CUBE_FEATURES_DOCUMENT,
      features: CUBE_FEATURES_DOCUMENT.features.slice(0, 3),
      rollback: 'f-ext1',
    }
    const fake = await startWithDocument('cube.ptsm', recipe)
    await startFeatureCommand(fake.app, 'box')
    await settle()
    const edits = fake.transport.commandsOfType('apply_edits').flatMap((command) => command.edits)
    const added = edits.find((edit) => edit.type === 'add_feature')
    expect(added?.type === 'add_feature' ? added.index : null).toBe(1)
    expect(edits.some((edit) => edit.type === 'set_rollback')).toBe(false)
  })

  it('cancels an active command before starting another', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await startFeatureCommand(fake.app, 'box')
    await settle()
    await startFeatureCommand(fake.app, 'cylinder')
    await settle()
    expect(relevantTypes(fake).filter((type) => type !== 'apply_edits' && type !== 'set_focus')).toEqual([
      'begin_transaction',
      'cancel_transaction',
      'begin_transaction',
    ])
    expect(fake.app.store.getState().command?.featureType).toBe('cylinder')
  })
})

describe('confirmCommand and cancelCommand (3.7)', () => {
  it('U-66 confirmCommand commits the transaction and clears the command', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    await confirmCommand(fake.app)
    await settle()
    expect(relevantTypes(fake).at(-1)).toBe('commit_transaction')
    expect(fake.app.store.getState().command).toBeNull()
    const recipe = currentDocument(fake.app).recipe
    expect(recipe.features.map((feature) => feature.type)).toEqual(['sketch', 'extrude'])
    expect(recipe.bodies.map((body) => body.name)).toEqual(['Body 1'])
    expect(currentDocument(fake.app).undoLabel).toBe('Extrude')
  })

  it('U-66 cancelCommand cancels the transaction, clears the command and records nothing', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    await cancelCommand(fake.app)
    await settle()
    expect(relevantTypes(fake).at(-1)).toBe('cancel_transaction')
    expect(fake.transport.commandTypes()).not.toContain('commit_transaction')
    expect(fake.app.store.getState().command).toBeNull()
    expect(currentDocument(fake.app).recipe).toEqual(RECT_SKETCH_DOCUMENT)
    expect(currentDocument(fake.app).undoLabel).toBeNull()
  })

  it('confirmCommand applies the latest params before committing', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    setCommandField(fake.app, 'extent.expr', '30 mm')
    await confirmCommand(fake.app)
    await settle()
    const types = relevantTypes(fake)
    expect(types.at(-1)).toBe('commit_transaction')
    expect(types.at(-2)).toBe('apply_edits')
    const extrude = currentDocument(fake.app).recipe.features[1]
    expect(extrude?.['extent']).toEqual({ kind: 'distance', expr: '30 mm' })
  })
})

describe('setCommandField and setActiveField', () => {
  it('a field change updates the params and previews the full record with update_feature', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    const featureId = fake.app.store.getState().command?.featureId
    setCommandField(fake.app, 'extent.expr', '25 mm')
    await settle()
    expect(fake.app.store.getState().command?.params).toMatchObject({
      extent: { kind: 'distance', expr: '25 mm' },
    })
    const updated = updatedFeatures(fake).at(-1)
    expect(updated).toMatchObject({
      id: featureId,
      type: 'extrude',
      extent: { kind: 'distance', expr: '25 mm' },
      profile: { sketch: 'f-sk1', regions: [RECT_REGION.reference] },
    })
  })

  it('setActiveField makes a selection field receive picks, or none', async () => {
    const fake = await rectSketchWithRegionSelected()
    await startFeatureCommand(fake.app, 'extrude')
    await settle()
    setActiveField(fake.app, 'body')
    expect(fake.app.store.getState().command?.activeField).toBe('body')
    setActiveField(fake.app, null)
    expect(fake.app.store.getState().command?.activeField).toBeNull()
  })

  it('capturePick toggles an accepted edge in the active field and fills the sibling body', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
    })
    await startFeatureCommand(fake.app, 'fillet')
    await settle()
    expect(fake.app.store.getState().command?.activeField).toBe('edges')
    const edge = elementItem(CUBE_TOP_FRONT_EDGE, 'Cube')
    await capturePick(fake.app, edge)
    await settle()
    expect(fake.app.store.getState().command?.params).toMatchObject({
      body: 'b-cube',
      edges: [CUBE_TOP_FRONT_EDGE.reference],
    })
    await capturePick(fake.app, edge)
    await settle()
    expect(fake.app.store.getState().command?.params['edges'] ?? []).toEqual([])
  })
})

describe('editFeature (3.7)', () => {
  it('U-67 begins "Edit Extrude 1" and focuses f-ext1, with the record as params', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    const doc = currentDocument(fake.app).documentHandle
    await editFeature(fake.app, 'f-ext1')
    await settle()
    expect(fake.transport.commandsOfType('begin_transaction')).toEqual([
      { type: 'begin_transaction', doc, label: 'Edit Extrude 1' },
    ])
    expect(fake.transport.commandsOfType('set_focus')).toEqual([
      { type: 'set_focus', doc, feature: 'f-ext1' },
    ])
    expect(fake.app.store.getState().command).toMatchObject({
      featureType: 'extrude',
      featureId: 'f-ext1',
      editing: true,
      applied: true,
      title: 'Extrude 1',
    })
    expect(fake.app.store.getState().command?.params).toMatchObject({
      body: 'b-cube',
      profile: CUBE_EXTRUDE['profile'],
      extent: { kind: 'distance', expr: '20 mm' },
      op: 'new',
    })
  })

  it('U-67 focus_inactive on a suppressed feature cancels the transaction and offers "Unsuppress"', async () => {
    const recipe: Document = {
      ...CUBE_FEATURES_DOCUMENT,
      features: CUBE_FEATURES_DOCUMENT.features.map((feature) =>
        feature.id === 'f-fil1' ? { ...feature, suppressed: true } : feature,
      ),
    }
    const message = 'Fillet 1 is not active. Move the rollback marker past it or unsuppress it first.'
    const fake = await startWithDocument('cube.ptsm', recipe)
    fake.transport.handle('set_focus', (command, core) => {
      const reply = core.reply(command)
      return 'error' in reply ? { error: { ...reply.error, message } } : reply
    })
    await editFeature(fake.app, 'f-fil1')
    await settle()
    expect(relevantTypes(fake)).toEqual(['begin_transaction', 'set_focus', 'cancel_transaction'])
    const notification = notificationWith(fake.app, message)
    expect(notification.severity).toBe('error')
    expect(actionLabels(notification)).toEqual(['Unsuppress'])
    expect(fake.app.store.getState().command).toBeNull()
  })

  it('"Unsuppress" unsuppresses the feature and retries the edit', async () => {
    const recipe: Document = {
      ...CUBE_FEATURES_DOCUMENT,
      features: CUBE_FEATURES_DOCUMENT.features.map((feature) =>
        feature.id === 'f-fil1' ? { ...feature, suppressed: true } : feature,
      ),
    }
    const fake = await startWithDocument('cube.ptsm', recipe)
    await editFeature(fake.app, 'f-fil1')
    await settle()
    const notification = fake.app.store
      .getState()
      .notifications.find((candidate) => candidate.severity === 'error')
    if (notification === undefined) {
      throw new Error('No error notification.')
    }
    fake.transport.clearRequests()
    runAction(notification, 'Unsuppress')
    await settle()
    const unsuppressed = updatedFeatures(fake).find((feature) => feature.id === 'f-fil1')
    expect(unsuppressed?.suppressed ?? false).toBe(false)
    expect(fake.transport.commandsOfType('begin_transaction').at(-1)?.label).toBe('Edit Fillet 1')
    expect(fake.app.store.getState().command?.featureId).toBe('f-fil1')
  })

  it('focus_inactive on a rolled-back feature offers "Move marker to the end", which clears the rollback and retries', async () => {
    const fake = await startWithDocument('cube.ptsm', { ...CUBE_FEATURES_DOCUMENT, rollback: 'f-fil1' })
    await editFeature(fake.app, 'f-fil1')
    await settle()
    const notification = fake.app.store
      .getState()
      .notifications.find((candidate) => candidate.severity === 'error')
    expect(notification === undefined ? [] : actionLabels(notification)).toEqual([
      'Move marker to the end',
    ])
    if (notification === undefined) {
      return
    }
    fake.transport.clearRequests()
    runAction(notification, 'Move marker to the end')
    await settle()
    const edits = fake.transport.commandsOfType('apply_edits').flatMap((command) => command.edits)
    expect(
      edits.some((edit) => edit.type === 'set_rollback' && (edit.rollback ?? null) === null),
    ).toBe(true)
    expect(currentDocument(fake.app).recipe.rollback ?? null).toBeNull()
    expect(fake.app.store.getState().command?.featureId).toBe('f-fil1')
  })

  it('editing a sketch enters sketch mode instead of the inspector', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT, {
      core: { solutions: { 'f-sk1': CUBE_SKETCH_SOLUTION } },
    })
    await editFeature(fake.app, 'f-sk1')
    await settle()
    const state = fake.app.store.getState()
    expect(state.context).toBe('sketch')
    expect(state.sketch?.sketchId).toBe('f-sk1')
    expect(state.command).toBeNull()
    expect(fake.transport.commandTypes()).not.toContain('begin_transaction')
  })

})
