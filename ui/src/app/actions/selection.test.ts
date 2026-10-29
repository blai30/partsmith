import { describe, expect, it } from 'vitest'
import type { PickResult } from '../../viewport/picking'
import { createTestApp, currentDocument, startWithDocument } from '../../test/fakes/app'
import { deferred } from '../../test/fakes/deferred'
import { CUBE_DOCUMENT, CUBE_FRONT_FACE, CUBE_TOP_FACE } from '../../test/fakes/documents'
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
