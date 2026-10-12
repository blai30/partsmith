import { describe, expect, it } from 'vitest'
import type { Diagnostic, FeatureStatus, FieldHint, Repair } from '../generated/protocol'
import {
  fieldForPointer,
  fieldPointer,
  messagesByField,
  repairField,
  suggestedRecord,
} from './status'

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

function outOfRange(field: string | null, suggestion: string | null): Diagnostic {
  return { severity: 'error', code: 'out_of_range', message: 'Too far.', field, suggestion }
}

describe('suggestedRecord', () => {
  const record = {
    id: 'f-ext1',
    type: 'extrude',
    extent: { kind: 'distance', expr: '30 mm' },
    sizes: ['1 mm', '2 mm'],
  }

  it('replaces the value at a nested pointer and keeps the rest of the record', () => {
    expect(suggestedRecord(record, outOfRange('/extent/expr', '20 mm'))).toEqual({
      ...record,
      extent: { kind: 'distance', expr: '20 mm' },
    })
  })

  it('replaces an array element', () => {
    expect(suggestedRecord(record, outOfRange('/sizes/1', '3 mm'))?.['sizes']).toEqual([
      '1 mm',
      '3 mm',
    ])
  })

  it('is null without a suggestion, without a field, or for a path not in the record', () => {
    expect(suggestedRecord(record, outOfRange('/extent/expr', null))).toBeNull()
    expect(suggestedRecord(record, outOfRange(null, '20 mm'))).toBeNull()
    expect(suggestedRecord(record, outOfRange('/missing/expr', '20 mm'))).toBeNull()
    expect(suggestedRecord(record, outOfRange('/sizes/5', '3 mm'))).toBeNull()
    expect(suggestedRecord(record, outOfRange('/id', 'f-other'))).toBeNull()
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
