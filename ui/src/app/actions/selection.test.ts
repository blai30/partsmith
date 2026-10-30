import { describe, expect, it } from 'vitest'
import { pickRow } from '../../layout/pick-row'
import { registerMissingCommands } from '../../layout/test-app.testing'
import type { PickResult } from '../../viewport/picking'
import {
  activeCommand,
  createTestApp,
  currentDocument,
  type FakeApp,
  startWithDocument,
} from '../../test/fakes/app'
import { deferred, type Deferred } from '../../test/fakes/deferred'
import {
  CUBE_DOCUMENT,
  CUBE_FRONT_FACE,
  CUBE_TOP_FACE,
  CUBE_TOP_FRONT_EDGE,
  featuresOf,
} from '../../test/fakes/documents'
import { editsOf, type ErrorReply } from '../../test/fakes/protocol'
import { bodyItem, elementItem, featureItem, originItem } from '../../test/fakes/selection'
import { settle } from '../../test/fakes/settle'
import { confirmCommand, startFeatureCommand } from './command'
import { clearSelection, pickAt, select, setHover, toggledByKey } from './selection'

const CUBE = bodyItem('b-cube', 'Cube')
const EXTRUDE = featureItem('f-ext1', 'Extrude 1')
const XY = originItem('XY')

function keys(items: readonly { readonly key: string }[]): string[] {
  return items.map((item) => item.key)
}

function meshPick(index: number): PickResult {
  return {
    kind: 'mesh',
    body: 'b-cube',
    meshId: 1,
    element: 'face',
    index,
    key: `mesh:1:face:${String(index)}`,
    world: [0, 0, 20],
  }
}

describe('select', () => {
  it('replace makes the items the selection', () => {
    const app = createTestApp()
    select(app, [CUBE], 'replace')
    select(app, [EXTRUDE, XY], 'replace')
    expect(keys(app.store.getState().selection)).toEqual(['feature:f-ext1', 'origin:XY'])
  })

  it('toggle adds items that are not selected and removes those that are, keeping the order', () => {
    const app = createTestApp()
    select(app, [CUBE, EXTRUDE], 'replace')
    select(app, [XY], 'toggle')
    expect(keys(app.store.getState().selection)).toEqual([
      'body:b-cube',
      'feature:f-ext1',
      'origin:XY',
    ])
    select(app, [EXTRUDE], 'toggle')
    expect(keys(app.store.getState().selection)).toEqual(['body:b-cube', 'origin:XY'])
  })

  it('clearSelection empties the selection', () => {
    const app = createTestApp()
    select(app, [CUBE], 'replace')
    clearSelection(app)
    expect(app.store.getState().selection).toEqual([])
  })
})

describe('toggledByKey', () => {
  it('removes selected items and adds the others after the ones already selected', () => {
    expect(keys(toggledByKey([CUBE, EXTRUDE], [XY, CUBE]))).toEqual(['feature:f-ext1', 'origin:XY'])
  })

  it('adds and removes an item listed twice', () => {
    expect(keys(toggledByKey([CUBE], [XY, XY]))).toEqual(['body:b-cube'])
  })
})

describe('setHover', () => {
  it('sets and clears the hover target', () => {
    const app = createTestApp()
    setHover(app, { key: 'mesh:1:face:3', kind: 'face' })
    expect(app.store.getState().hover).toEqual({ key: 'mesh:1:face:3', kind: 'face' })
    setHover(app, null)
    expect(app.store.getState().hover).toBeNull()
  })
})

describe('pickAt (3.6)', () => {
  it('a mesh pick asks pick_reference for the element and selects it under its element key', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    fake.transport.handle('pick_reference', () => ({
      result: { type: 'pick_reference', element: CUBE_TOP_FACE },
    }))
    await pickAt(fake.app, meshPick(3), { toggle: false })
    await settle()
    expect(fake.transport.commandsOfType('pick_reference')).toEqual([
      {
        type: 'pick_reference',
        doc: currentDocument(fake.app).documentHandle,
        pick: { mesh_id: 1, kind: 'face', index: 3 },
      },
    ])
    const [item] = fake.app.store.getState().selection
    expect(item).toMatchObject({
      key: 'element:b-cube:f-ext1/end-cap',
      kind: 'face',
      body: 'b-cube',
      feature: 'f-ext1',
      element: CUBE_TOP_FACE,
    })
    expect(fake.app.store.getState().selection).toHaveLength(1)
  })

  it('with toggle the picked element is added to the selection', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [elementItem(CUBE_FRONT_FACE, 'Cube')], 'replace')
    fake.transport.handle('pick_reference', () => ({
      result: { type: 'pick_reference', element: CUBE_TOP_FACE },
    }))
    await pickAt(fake.app, meshPick(3), { toggle: true })
    await settle()
    expect(keys(fake.app.store.getState().selection)).toEqual([
      'element:b-cube:f-ext1/side-0',
      'element:b-cube:f-ext1/end-cap',
    ])
  })

  it.each(['stale_pick', 'preview_not_pickable'] as const)(
    'a %s answer ignores the click silently',
    async (code) => {
      const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
      select(fake.app, [CUBE], 'replace')
      fake.transport.handle('pick_reference', () => ({
        error: { code, message: 'The picked geometry changed since it was displayed. Pick again.' },
      }))
      await pickAt(fake.app, meshPick(3), { toggle: false })
      await settle()
      expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
      expect(fake.app.store.getState().notifications).toEqual([])
    },
  )

  it('an item pick selects the item without a core call', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    await pickAt(fake.app, { kind: 'item', item: XY, world: null }, { toggle: false })
    await settle()
    expect(keys(fake.app.store.getState().selection)).toEqual(['origin:XY'])
    expect(fake.transport.commandTypes()).not.toContain('pick_reference')
  })
})

describe('pickAt while OK runs (3.7)', () => {
  it('a click selects once the command has closed instead of going into it', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [XY], 'replace')
    await startFeatureCommand(fake.app, 'datum_plane')
    await settle()
    const commit = deferred<undefined>()
    fake.transport.handle('commit_transaction', async (command, core, buffers) => {
      await commit.promise
      return core.reply(command, buffers)
    })
    const confirmed = confirmCommand(fake.app)
    await settle()
    const click = pickAt(
      fake.app,
      { kind: 'item', item: originItem('YZ'), world: null },
      { toggle: false },
    )
    commit.resolve(undefined)
    await confirmed
    await click
    expect(fake.app.store.getState().command).toBeNull()
    expect(keys(fake.app.store.getState().selection)).toEqual(['origin:YZ'])
  })
})

/** The core's answer to a held pick_reference: the cube's top face (null), or a refusal. */
type PickAnswer = ErrorReply | null

/**
 * Holds the core's answer to every pick_reference until the test resolves the returned promise
 * with null (the cube's top face) or a refusal. The fake core answers in request order, so every
 * later request waits too, as behind a core that answers late.
 */
function holdPicks(fake: FakeApp): Deferred<PickAnswer> {
  const held = deferred<PickAnswer>()
  fake.transport.handle('pick_reference', async () => {
    const refusal = await held.promise
    return refusal === null
      ? { result: { type: 'pick_reference', element: CUBE_TOP_FACE } }
      : { error: refusal }
  })
  return held
}

const TOP_FACE_KEY = `element:b-cube:${CUBE_TOP_FACE.name}`

/** cube.ptsm with a click on the top face on its way: the core has not answered its pick yet. */
async function topFaceClickOnItsWay(): Promise<{
  readonly fake: FakeApp
  readonly picked: Deferred<PickAnswer>
  readonly click: Promise<void>
}> {
  const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
  const picked = holdPicks(fake)
  const click = pickAt(fake.app, meshPick(0), { toggle: false })
  await settle()
  expect(fake.transport.commandTypes()).toContain('pick_reference')
  return { fake, picked, click }
}

/** Answers the held pick with the top face and lets everything that waited for it land. */
async function answerTopFace(picked: Deferred<PickAnswer>, click: Promise<void>): Promise<void> {
  picked.resolve(null)
  await click
  await settle()
}

describe('U-241 clicks and selection changes land in the order they were made (3.6)', () => {
  it('U-241 (a) "Select Cube" right after a click on a face selects Cube, however late the core answers the face', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    select(fake.app, [CUBE], 'replace')
    await answerTopFace(picked, click)
    expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
  })

  it('U-241 (b) Clear selection right after a click on a face leaves nothing selected', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    clearSelection(fake.app)
    await answerTopFace(picked, click)
    expect(fake.app.store.getState().selection).toEqual([])
  })

  it('U-241 (c) a click on the browser row "Cube" right after a click on a face selects the body alone', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    pickRow(fake.app, CUBE, false)
    await answerTopFace(picked, click)
    expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
  })

  it('U-241 (d) a toggle of the body right after a click on a face adds the body after the face', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    select(fake.app, [CUBE], 'toggle')
    await answerTopFace(picked, click)
    expect(keys(fake.app.store.getState().selection)).toEqual([TOP_FACE_KEY, 'body:b-cube'])
  })

  it('U-241 (e) guard: with no click on its way, a selection change is made at once', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    select(fake.app, [CUBE], 'replace')
    expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
  })

  it('U-241 (f) an item pick that needs no core call, toggled right after a click on a face, lands after the face', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    const itemClick = pickAt(fake.app, { kind: 'item', item: XY, world: null }, { toggle: true })
    picked.resolve(null)
    await Promise.all([click, itemClick])
    await settle()
    expect(keys(fake.app.store.getState().selection)).toEqual([TOP_FACE_KEY, 'origin:XY'])
  })

  it('U-241 (g) guard: a click the core refuses lands nothing, shows the error, and the change that waited for it lands', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    select(fake.app, [CUBE], 'replace')
    picked.resolve({ code: 'internal_error', message: 'The pick could not be read.' })
    await click
    await settle()
    expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
    expect(fake.app.store.getState().notifications).toContainEqual(
      expect.objectContaining({ severity: 'error', message: 'The pick could not be read.' }),
    )
  })

  it('U-241 (g) guard: a click a stopped core rejects lands nothing, and the change that waited for it lands', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    select(fake.app, [CUBE], 'replace')
    fake.transport.die('The geometry core stopped.')
    // The answer the core was working on never arrives: the request rejects with CoreFatalError.
    picked.resolve(null)
    await click
    await settle()
    expect(keys(fake.app.store.getState().selection)).toEqual(['body:b-cube'])
    expect(fake.app.store.getState().notifications).toEqual([])
  })

  it('U-241 (h) guard: Delete pressed after a click on a face and a click on the row "Cube" deletes Cube', async () => {
    const fake = await startWithDocument('cube.ptsm', CUBE_DOCUMENT)
    registerMissingCommands(fake.app)
    select(fake.app, [featureItem('f-sk1', 'Sketch 1')], 'replace')
    const picked = holdPicks(fake)
    const click = pickAt(fake.app, meshPick(0), { toggle: false })
    pickRow(fake.app, CUBE, false)
    const deleted = fake.app.commands.run(fake.app, 'edit.delete', 'shortcut')
    picked.resolve(null)
    await Promise.all([click, deleted])
    await settle()
    const changes = fake.transport.commandsOfType('apply_edits')
    expect(changes.map((change) => change.label)).toEqual(['Delete Cube'])
    expect(editsOf(changes[0])).toEqual([{ type: 'remove_body', id: 'b-cube' }])
    expect(featuresOf(currentDocument(fake.app).recipe).map((record) => record.id)).toContain(
      'f-sk1',
    )
  })

  it('U-241 (i) a feature start preselects the selection change made after a click on its way', async () => {
    const { fake, picked, click } = await topFaceClickOnItsWay()
    select(fake.app, [elementItem(CUBE_TOP_FRONT_EDGE, 'Cube')], 'replace')
    const started = startFeatureCommand(fake.app, 'fillet')
    picked.resolve(null)
    await Promise.all([click, started])
    await settle()
    expect(activeCommand(fake.app)).toMatchObject({ featureType: 'fillet', history: [] })
    expect(activeCommand(fake.app).params['edges']).toEqual([CUBE_TOP_FRONT_EDGE.reference])
    expect(fake.app.store.getState().selection).toEqual([])
  })
})
