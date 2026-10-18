import { describe, expect, it } from 'vitest'
import type { Diagnostic, FeatureStatus, FieldHint, Repair } from '../generated/protocol'
import { fieldForPointer, fieldLimit, fieldPointer, messagesByField, repairField } from './status'

describe('fieldPointer', () => {
  it('turns a hint path into a JSON pointer', () => {
    expect(fieldPointer('radius')).toBe('/radius')
    expect(fieldPointer('extent.expr')).toBe('/extent/expr')
    expect(fieldPointer('a.b[1]')).toBe('/a/b/1')
  })
})

describe('fieldForPointer', () => {
  const paths = ['edges', 'radius', 'extent.kind', 'extent.expr']

  it('maps an exact pointer to its field', () => {
    expect(fieldForPointer(paths, '/radius')).toBe('radius')
    expect(fieldForPointer(paths, '/extent/expr')).toBe('extent.expr')
  })

  it('maps a pointer below a field to the field with the longest whole-segment prefix', () => {
    expect(fieldForPointer(paths, '/edges/1')).toBe('edges')
  })

  it('does not match a field that is only a prefix of a segment', () => {
    expect(fieldForPointer(['edge'], '/edges/1')).toBeNull()
  })

  it('maps nothing for an unknown pointer', () => {
    expect(fieldForPointer(paths, '/body')).toBeNull()
  })
})

describe('messagesByField', () => {
  it('keeps the first message per field and skips messages without a field', () => {
    const first: Diagnostic = { severity: 'error', code: 'a', message: 'First.', field: '/radius' }
    const second: Diagnostic = {
      severity: 'warning',
      code: 'b',
      message: 'Second.',
      field: '/radius',
    }
    const general: Diagnostic = { severity: 'warning', code: 'c', message: 'General.' }
    const byField = messagesByField(['radius'], [general, first, second])
    expect(byField.get('radius')).toBe(first)
    expect(byField.size).toBe(1)
  })
})

function statusWith(repairs: readonly Repair[]): FeatureStatus {
  return { id: 'f-fil1', status: 'error', messages: [], repairs: [...repairs] }
}

describe('repairField (4.2)', () => {
  const fields: readonly FieldHint[] = [
    { path: 'body', label: 'Body', selection: 'body' },
    { path: 'edges', label: 'Edges', selection: 'edge' },
    { path: 'radius', label: 'Radius', unit: 'length' },
  ]
  const lostEdge: Repair = {
    kind: 'element',
    path: '/edges/1',
    body: 'b-cube',
    reference: { q: 'edge(top,front)', fp: 'fp-edge' },
    resolution: 'missing',
    description: "the front top edge of 'Extrude 1'",
    candidates: [],
  }

  it('U-110 maps the first repair to the selection field its path is under', () => {
    expect(repairField(fields, statusWith([lostEdge]))?.label).toBe('Edges')
  })

  it('maps a region repair to the profile field', () => {
    const region: Repair = {
      kind: 'region',
      path: '/profile/regions/0',
      sketch: 'f-sk1',
      region: { key: 'e-1', fp: 'fp-region' },
      resolution: 'ambiguous',
      description: "the region of 'Sketch 1' around (0, 0)",
      candidates: [],
    }
    const profile: FieldHint = { path: 'profile', label: 'Profile', selection: 'sketch_region' }
    expect(repairField([profile], statusWith([region]))?.path).toBe('profile')
  })

  it('is null without repairs, without a status or for a path under no selection field', () => {
    expect(repairField(fields, statusWith([]))).toBeNull()
    expect(repairField(fields, undefined)).toBeNull()
    expect(repairField(fields, statusWith([{ ...lostEdge, path: '/radius' }]))).toBeNull()
  })
})

describe('fieldLimit (3.7)', () => {
  const tooLarge: Diagnostic = {
    severity: 'error',
    code: 'fillet_too_large',
    message: 'Fillet radius 3 mm is too large for the selected edges.',
    field: '/radius',
    suggestion: '2.3 mm',
  }

  it.each(['fillet_too_large', 'chamfer_too_large', 'shell_too_thick'])(
    'is the suggestion of %s',
    (code) => {
      expect(fieldLimit({ ...tooLarge, code }, '3 mm', 'mm')).toBe('2.3 mm')
    },
  )

  it('is null without a message, a suggestion or a "too large" code', () => {
    const { suggestion: _suggestion, ...withoutSuggestion } = tooLarge
    expect(fieldLimit(undefined, '3 mm', 'mm')).toBeNull()
    expect(fieldLimit(withoutSuggestion, '3 mm', 'mm')).toBeNull()
    expect(fieldLimit({ ...tooLarge, suggestion: null }, '3 mm', 'mm')).toBeNull()
    expect(fieldLimit({ ...tooLarge, code: 'out_of_range' }, '3 mm', 'mm')).toBeNull()
  })

  it('is null for a number without a unit in a document whose units are not mm', () => {
    expect(fieldLimit({ ...tooLarge, suggestion: '2 mm' }, '2', 'in')).toBeNull()
    expect(fieldLimit({ ...tooLarge, suggestion: '1.9 in' }, '2 in', 'in')).toBe('1.9 in')
    expect(fieldLimit(tooLarge, '3', 'mm')).toBe('2.3 mm')
    expect(fieldLimit({ ...tooLarge, suggestion: '1.9 in' }, 'r * 2', 'in')).toBe('1.9 in')
  })
})
