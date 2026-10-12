import { describe, expect, it } from 'vitest'
import { formatArea } from './area'

describe('formatArea', () => {
  it('writes mm² in a millimeter document', () => {
    expect(formatArea(400, 'mm')).toBe('400 mm²')
  })

  it('converts to the document unit squared', () => {
    expect(formatArea(400, 'cm')).toBe('4 cm²')
    expect(formatArea(645.16, 'in')).toBe('1 in²')
  })
})
