import { describe, expect, it } from 'vitest'
import { cubeDocumentState, readyState } from '../layout/fixtures.testing'
import { INITIAL_STATE, type AppState } from '../state/store'
import { GENERIC_RESIN } from '../test/fakes/printers'
import {
  completionAt,
  completionNames,
  insertCompletion,
  missingParameterName,
  PARAMETER_NAME,
  type CompletionName,
} from './completion'

// Parameter name completion (ui.md 2.13): which identifier the caret ends and which names match.

const NAMES: readonly CompletionName[] = [
  { name: 'size', display: 'size = 20 mm' },
  { name: 'slot', display: 'slot = 3 mm' },
  { name: 'printer.nozzle', display: 'printer.nozzle' },
]

/** cube.ptsm with the parameter `size` = 20 mm. */
function sizedState(): AppState {
  const document = cubeDocumentState()
  return {
    ...INITIAL_STATE,
    ...readyState({
      ...document,
      recipe: { ...document.recipe, parameters: [{ id: 'p-size', name: 'size', expr: '20 mm' }] },
      parameterValues: {
        'p-size': { id: 'p-size', value: 20, dimension: { length: 1, angle: 0 } },
      },
    }),
  }
}

describe('completionAt', () => {
  it('matches the identifier the caret ends, best first', () => {
    expect(completionAt('2 * si', 6, NAMES)).toEqual({
      start: 4,
      end: 6,
      identifier: 'si',
      options: [NAMES[0]],
    })
  })

  it('stays closed inside an identifier, after a number, and for reserved words', () => {
    expect(completionAt('size2', 2, NAMES)).toBeNull()
    expect(completionAt('10 mm', 5, NAMES)).toBeNull()
    expect(completionAt('10mm', 4, NAMES)).toBeNull()
    expect(completionAt('2 * pi', 6, NAMES)).toBeNull()
    expect(completionAt('sqrt', 4, NAMES)).toBeNull()
  })

  it('stays closed for a name typed in full and for one no name matches', () => {
    expect(completionAt('size', 4, NAMES)).toBeNull()
    expect(completionAt('xyz', 3, NAMES)).toBeNull()
  })

  it('takes dotted printer names', () => {
    expect(completionAt('printer.n', 9, NAMES)?.options[0]?.name).toBe('printer.nozzle')
  })
})

describe('insertCompletion', () => {
  it('replaces the identifier and puts the caret after the name', () => {
    const completion = completionAt('2 * sl + 1', 6, NAMES)
    if (completion === null) {
      throw new Error('No completion.')
    }
    expect(insertCompletion('2 * sl + 1', completion, 'slot')).toEqual({
      text: '2 * slot + 1',
      caret: 8,
    })
  })
})

describe('completionNames', () => {
  it('lists parameters with their values, then the printer names of an FDM profile', () => {
    const names = completionNames(sizedState())
    expect(names[0]).toEqual({ name: 'size', display: 'size = 20 mm' })
    expect(names.map((entry) => entry.name)).toEqual([
      'size',
      'printer.fit.press',
      'printer.fit.snug',
      'printer.fit.slip',
      'printer.fit.loose',
      'printer.layer_height',
      'printer.nozzle',
    ])
  })

  it('leaves out printer.nozzle for a resin profile', () => {
    const state = sizedState()
    const document = state.document
    if (document === null) {
      throw new Error('No document.')
    }
    const resin = {
      ...state,
      document: { ...document, recipe: { ...document.recipe, printer: GENERIC_RESIN.id } },
    }
    expect(completionNames(resin).map((entry) => entry.name)).not.toContain('printer.nozzle')
  })
})

describe('missingParameterName', () => {
  it('names an identifier that no parameter has', () => {
    expect(missingParameterName('2 * wall', { start: 4, end: 8 }, sizedState())).toBe('wall')
  })

  it('names nothing for a parameter, a reserved word, a dotted name or no range', () => {
    expect(missingParameterName('size', { start: 0, end: 4 }, sizedState())).toBeNull()
    expect(missingParameterName('pi', { start: 0, end: 2 }, sizedState())).toBeNull()
    expect(missingParameterName('printer.x', { start: 0, end: 9 }, sizedState())).toBeNull()
    expect(missingParameterName('wall', null, sizedState())).toBeNull()
  })
})

describe('PARAMETER_NAME', () => {
  it('takes a letter or underscore, then letters, digits and underscores', () => {
    for (const name of ['width', '_gap', 'wall_2', 'A1']) {
      expect(PARAMETER_NAME.test(name)).toBe(true)
    }
  })

  it('refuses names that start with a digit or hold other characters', () => {
    for (const name of ['2nd', 'wall-2', 'printer.nozzle', '', 'a b']) {
      expect(PARAMETER_NAME.test(name)).toBe(false)
    }
  })
})
