import { describe, expect, it } from 'vitest'
import { formatNumber, formatPoint, formatQuantity, formatSize } from './format'

describe('formatQuantity', () => {
  it('U-39 12.5 mm is "12.5 mm"', () => {
    expect(formatQuantity(12.5, 'length', 'mm')).toBe('12.5 mm')
  })

  it('U-39 2.4 mm in an inch document is "0.0945 in"', () => {
    expect(formatQuantity(2.4, 'length', 'in')).toBe('0.0945 in')
  })

  it('U-39 1/3 mm is "0.333 mm"', () => {
    expect(formatQuantity(1 / 3, 'length', 'mm')).toBe('0.333 mm')
  })

  it('U-39 -0 mm is "0 mm"', () => {
    expect(formatQuantity(-0, 'length', 'mm')).toBe('0 mm')
  })

  it('U-39 1234.5 mm is "1234.5 mm" with no digit grouping', () => {
    expect(formatQuantity(1234.5, 'length', 'mm')).toBe('1234.5 mm')
  })

  it('U-39 25.4 mm in a centimeter document is "2.54 cm"', () => {
    expect(formatQuantity(25.4, 'length', 'cm')).toBe('2.54 cm')
  })

  it('U-39 1 mm in a meter document is "0.001 m"', () => {
    expect(formatQuantity(1, 'length', 'm')).toBe('0.001 m')
  })

  it('U-39 π/4 rad is "45°"', () => {
    expect(formatQuantity(Math.PI / 4, 'angle', 'mm')).toBe('45°')
  })

  it('appends no suffix to scalars and counts', () => {
    expect(formatQuantity(2.5, 'scalar', 'mm')).toBe('2.5')
    expect(formatQuantity(4, 'count', 'in')).toBe('4')
  })
})

describe('formatNumber', () => {
  it('writes a length without its unit', () => {
    expect(formatNumber(12.5, 'length', 'mm')).toBe('12.5')
  })

  it('rounds millimeters to 3 decimals', () => {
    expect(formatNumber(2.00049, 'length', 'mm')).toBe('2')
    expect(formatNumber(2.0006, 'length', 'mm')).toBe('2.001')
  })

  it('rounds centimeters to 4 decimals', () => {
    expect(formatNumber(12.345678, 'length', 'cm')).toBe('1.2346')
  })

  it('rounds meters to 6 decimals', () => {
    expect(formatNumber(1234.5678, 'length', 'm')).toBe('1.234568')
  })

  it('rounds inches to 4 decimals', () => {
    expect(formatNumber(25.4, 'length', 'in')).toBe('1')
    expect(formatNumber(10, 'length', 'in')).toBe('0.3937')
  })

  it('writes a value that rounds to -0 as 0', () => {
    expect(formatNumber(-0.0001, 'length', 'mm')).toBe('0')
  })

  it('keeps the sign of negative lengths', () => {
    expect(formatNumber(-12.5, 'length', 'mm')).toBe('-12.5')
  })

  it('writes large lengths without digit grouping or exponent', () => {
    expect(formatNumber(1234567, 'length', 'mm')).toBe('1234567')
  })

  it('converts angles to degrees rounded to 2 decimals', () => {
    expect(formatNumber(Math.PI / 6, 'angle', 'mm')).toBe('30')
    expect(formatNumber(1, 'angle', 'mm')).toBe('57.3')
    expect(formatNumber(-Math.PI / 2, 'angle', 'mm')).toBe('-90')
  })

  it('writes angles the same in every document unit', () => {
    expect(formatNumber(Math.PI, 'angle', 'in')).toBe('180')
  })

  it('writes scalars with up to 6 significant digits', () => {
    expect(formatNumber(1 / 3, 'scalar', 'mm')).toBe('0.333333')
    expect(formatNumber(123456.7, 'scalar', 'mm')).toBe('123457')
    expect(formatNumber(2.5, 'scalar', 'mm')).toBe('2.5')
    expect(formatNumber(2, 'scalar', 'mm')).toBe('2')
  })

  it('writes counts as integers', () => {
    expect(formatNumber(4, 'count', 'mm')).toBe('4')
    expect(formatNumber(0, 'count', 'mm')).toBe('0')
  })
})

describe('formatSize', () => {
  it('U-39 the box (0, 0, 0) to (40, 20, 10) is "40 × 20 × 10 mm"', () => {
    expect(formatSize({ min: [0, 0, 0], max: [40, 20, 10] }, 'mm')).toBe('40 × 20 × 10 mm')
  })

  it('measures the extents of a box away from the origin', () => {
    expect(formatSize({ min: [-10, -10, 0], max: [10, 10, 20] }, 'mm')).toBe('20 × 20 × 20 mm')
  })

  it('converts to the document unit', () => {
    expect(formatSize({ min: [0, 0, 0], max: [25.4, 50.8, 12.7] }, 'in')).toBe('1 × 2 × 0.5 in')
  })
})

describe('formatPoint', () => {
  it('U-39 (10, 0, 10) is "10, 0, 10 mm"', () => {
    expect(formatPoint([10, 0, 10], 'mm')).toBe('10, 0, 10 mm')
  })

  it('converts to the document unit and writes -0 as 0', () => {
    expect(formatPoint([10, -5, -0], 'cm')).toBe('1, -0.5, 0 cm')
  })
})
