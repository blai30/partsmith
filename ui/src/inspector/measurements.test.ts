import { describe, expect, it } from 'vitest'
import type { ElementDescriptor } from '../generated/protocol'
import {
  CIRCULAR_EDGE,
  CORNER_VERTEX,
  cubeDocumentState,
  CYLINDRICAL_FACE,
  readyState,
} from '../layout/fixtures.testing'
import { createModelApp } from '../layout/test-app.testing'
import type { DocumentState, SelectionItem } from '../state/store'
import { CUBE_DOCUMENT } from '../test/fakes/documents'
import { bodyItem, elementItem } from '../test/fakes/selection'
import { formatVolume, measureSelection, selectionMeasurements } from './measurements'

// Measurements of the selection (ui.md 3.8).

const SPHERICAL_FACE: ElementDescriptor = {
  ...CYLINDRICAL_FACE,
  name: 'f-sph1/surface',
  reference: { q: 'f-sph1/surface', fp: 'sphere|c=0,0,0|r=5' },
  surface: 'sphere',
  radius: 5,
}

function appWith(document: DocumentState = cubeDocumentState(), volume = 8000) {
  const test = createModelApp({ state: readyState(document) })
  test.core.stub('measure', () => Promise.resolve({ volume }))
  return test
}

function rows(items: readonly SelectionItem[], document?: DocumentState) {
  const { app } = appWith(document)
  return selectionMeasurements(app, items).map(({ label, value }) => [label, value])
}

describe('selectionMeasurements (U-116)', () => {
  it('U-116 gives a circular edge of radius 2.5 the Diameter "5 mm"', () => {
    expect(rows([elementItem({ ...CIRCULAR_EDGE, radius: 2.5 }, 'Peg')])).toEqual([
      ['Diameter', '5 mm'],
    ])
  })

  it('U-116 gives a cylindrical face of radius 4 the Diameter "8 mm"', () => {
    expect(rows([elementItem(CYLINDRICAL_FACE, 'Peg')])).toEqual([['Diameter', '8 mm']])
  })

  it('U-116 gives a spherical face of radius 5 the Diameter "10 mm"', () => {
    expect(rows([elementItem(SPHERICAL_FACE, 'Ball')])).toEqual([['Diameter', '10 mm']])
  })

  it('U-116 gives a body the Volume "8000 mm³" that measure returns, empty until then', async () => {
    const { app, core } = appWith()
    const items = [bodyItem('b-cube', 'Cube')]
    expect(selectionMeasurements(app, items)).toEqual([
      { measure: 'volume', label: 'Volume', value: '' },
    ])
    await measureSelection(app, items)
    expect(core.callsTo('measure')).toEqual([[1, { kind: 'body', body: 'b-cube' }]])
    expect(selectionMeasurements(app, items)).toEqual([
      { measure: 'volume', label: 'Volume', value: '8000 mm³' },
    ])
  })

  it('U-116 writes the volume in an inch document as "0.4882 in³"', async () => {
    const inches = cubeDocumentState({ recipe: { ...CUBE_DOCUMENT, units: 'in' } })
    const { app } = appWith(inches)
    const items = [bodyItem('b-cube', 'Cube')]
    await measureSelection(app, items)
    expect(selectionMeasurements(app, items)[0]?.value).toBe('0.4882 in³')
  })

  it('U-116 gives a body whose Size is "Empty" no Volume row and calls no measure', async () => {
    const { app, core } = appWith(cubeDocumentState({ meshes: {} }))
    const items = [bodyItem('b-cube', 'Cube')]
    expect(selectionMeasurements(app, items)).toEqual([])
    await measureSelection(app, items)
    expect(core.callsTo('measure')).toEqual([])
  })

  it('U-116 gives two selected vertices no rows', () => {
    const other = { ...CORNER_VERTEX, name: 'vertex(other)' }
    expect(rows([elementItem(CORNER_VERTEX), elementItem(other)])).toEqual([])
  })

  it('measures one mesh once', async () => {
    const { app, core } = appWith()
    const items = [bodyItem('b-cube', 'Cube')]
    await measureSelection(app, items)
    await measureSelection(app, items)
    expect(core.callsTo('measure')).toHaveLength(1)
  })

  it('gives a straight edge no diameter', () => {
    expect(rows([elementItem({ ...CIRCULAR_EDGE, curve: 'line', radius: null }, 'Peg')])).toEqual(
      [],
    )
  })
})

describe('formatVolume', () => {
  it('writes the volume in the document unit cubed, rounded as a length', () => {
    expect(formatVolume(8000, 'mm')).toBe('8000 mm³')
    expect(formatVolume(8000, 'cm')).toBe('8 cm³')
    expect(formatVolume(8000, 'in')).toBe('0.4882 in³')
  })
})
