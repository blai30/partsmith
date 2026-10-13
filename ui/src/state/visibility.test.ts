import { describe, expect, it } from 'vitest'
import { CUBE_DOCUMENT, CUBE_EXTRUDE, CUBE_SKETCH } from '../test/fakes/documents'
import { stateWithDocument } from '../test/fakes/state'
import {
  initialSessionVisibility,
  isSessionVisible,
  referencedSketches,
  referencesSketch,
  sketchStartsVisible,
} from './visibility'

const HOLE = {
  id: 'f-hole1',
  type: 'hole',
  points: [{ sketch: 'f-sk2', point: 'e-1' }],
  axis: { kind: 'sketch_line', sketch: 'f-sk1', line: 'e-1' },
}

describe('referencesSketch', () => {
  it('finds a sketch key at any depth, in objects and lists', () => {
    expect(referencesSketch(CUBE_EXTRUDE, 'f-sk1')).toBe(true)
    expect(referencesSketch(HOLE, 'f-sk2')).toBe(true)
    expect(referencesSketch(HOLE, 'f-sk3')).toBe(false)
    expect(referencesSketch('f-sk1', 'f-sk1')).toBe(false)
  })
})

describe('referencedSketches', () => {
  it('lists every sketch a value refers to, in key order', () => {
    expect(referencedSketches(HOLE)).toEqual(['f-sk2', 'f-sk1'])
    expect(referencedSketches(CUBE_SKETCH)).toEqual([])
  })
})

describe('sketchStartsVisible', () => {
  it('is false for a sketch another feature uses and true otherwise', () => {
    expect(sketchStartsVisible([CUBE_SKETCH, CUBE_EXTRUDE], 'f-sk1')).toBe(false)
    expect(sketchStartsVisible([CUBE_SKETCH], 'f-sk1')).toBe(true)
  })
})

describe('initialSessionVisibility', () => {
  it('hides used sketches and gives other features no entry', () => {
    expect(initialSessionVisibility(CUBE_DOCUMENT)).toEqual({ 'feature:f-sk1': false })
    expect(initialSessionVisibility(null)).toEqual({})
  })
})

describe('isSessionVisible', () => {
  it('follows a stored entry first', () => {
    const state = stateWithDocument(CUBE_DOCUMENT)
    const shown = { ...state, view: { ...state.view, sessionVisible: { 'feature:f-sk1': true } } }
    expect(isSessionVisible(shown, 'feature:f-sk1')).toBe(true)
  })

  it('without an entry, applies the opening rule to sketches and shows other features', () => {
    const state = stateWithDocument(CUBE_DOCUMENT)
    expect(isSessionVisible(state, 'feature:f-sk1')).toBe(false)
    expect(isSessionVisible(state, 'feature:f-ext1')).toBe(true)
  })

  it('without an entry, follows the document for origin items', () => {
    const state = stateWithDocument(CUBE_DOCUMENT)
    expect(isSessionVisible(state, 'origin:O')).toBe(false)
    expect(isSessionVisible(state, 'origin:XY')).toBe(false)
  })
})
