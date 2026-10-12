import { describe, expect, it } from 'vitest'
import type { Request } from '../generated/protocol'
import { isSuperseded } from './superseding'

function drag(id: number, feature = 'f-sk1', doc = 1): Request {
  return { id, command: { type: 'drag_sketch', doc, feature, point: 'e-1.end', to: [20, 5] } }
}

function solve(id: number, feature = 'f-sk1', doc = 1): Request {
  return { id, command: { type: 'solve_sketch', doc, feature } }
}

function analyze(id: number, doc = 1): Request {
  return { id, command: { type: 'analyze_print', doc, orientation: { kind: 'as_modeled' } } }
}

function edit(id: number, doc = 1): Request {
  return {
    id,
    command: {
      type: 'apply_edits',
      doc,
      label: 'Change units to cm',
      edits: [{ type: 'set_units', units: 'cm' }],
    },
  }
}

function superseded(queue: readonly Request[]): boolean[] {
  return queue.map((_request, index) => isSuperseded(queue, index))
}

describe('isSuperseded (protocol test 69)', () => {
  it('a drag_sketch is superseded by a later one for the same doc and feature', () => {
    expect(superseded([drag(10), drag(11)])).toEqual([true, false])
  })

  it('drags of different sketches do not supersede each other', () => {
    expect(superseded([drag(10, 'f-sk1'), drag(11, 'f-sk2')])).toEqual([false, false])
  })

  it('a request for the same doc between two drags keeps both', () => {
    expect(superseded([drag(10), edit(11), drag(12)])).toEqual([false, false, false])
  })

  it('an analyze_print is superseded by any later one for the same doc', () => {
    expect(superseded([analyze(20), edit(21), analyze(22)])).toEqual([true, false, false])
  })

  it('a solve_sketch is superseded by a later one for the same doc and feature', () => {
    expect(superseded([solve(30), solve(31)])).toEqual([true, false])
  })

  it('a request for the same doc between two solves keeps both', () => {
    expect(superseded([solve(30), edit(31), solve(32)])).toEqual([false, false, false])
  })

  it('a request for another doc between two drags does not keep the first', () => {
    expect(superseded([drag(10, 'f-sk1', 1), edit(11, 2), drag(12, 'f-sk1', 1)])).toEqual([
      true,
      false,
      false,
    ])
  })

  it('drags and analyses of another document never supersede', () => {
    expect(superseded([drag(10, 'f-sk1', 1), drag(11, 'f-sk1', 2)])).toEqual([false, false])
    expect(superseded([analyze(20, 1), analyze(21, 2)])).toEqual([false, false])
  })

  it('a drag is not superseded by a solve of the same sketch, nor a solve by a drag', () => {
    expect(superseded([drag(10), solve(11)])).toEqual([false, false])
    expect(superseded([solve(10), drag(11)])).toEqual([false, false])
  })

  it('nothing else is ever superseded', () => {
    expect(superseded([edit(1), edit(2)])).toEqual([false, false])
  })
})
