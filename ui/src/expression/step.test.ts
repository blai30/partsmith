import { describe, expect, it } from 'vitest'
import { isPlainNumber, stepExpression } from './step'

describe('stepExpression', () => {
  it('U-40 10 up gives 11', () => {
    expect(stepExpression('10', 1, 'none', 'length', 'mm')).toBe('11')
  })

  it('U-40 "10 mm" up gives "11 mm"', () => {
    expect(stepExpression('10 mm', 1, 'none', 'length', 'mm')).toBe('11 mm')
  })

  it('U-40 "10mm" up gives "11mm"', () => {
    expect(stepExpression('10mm', 1, 'none', 'length', 'mm')).toBe('11mm')
  })

  it('U-40 2.5 up coarse gives 12.5', () => {
    expect(stepExpression('2.5', 1, 'coarse', 'length', 'mm')).toBe('12.5')
  })

  it('U-40 2.5 up fine gives 2.6', () => {
    expect(stepExpression('2.5', 1, 'fine', 'length', 'mm')).toBe('2.6')
  })

  it('U-40 -0.1 up fine gives 0', () => {
    expect(stepExpression('-0.1', 1, 'fine', 'length', 'mm')).toBe('0')
  })

  it('U-40 wall gives null', () => {
    expect(stepExpression('wall', 1, 'none', 'length', 'mm')).toBeNull()
  })

  it('U-40 a count 0 down gives 0', () => {
    expect(stepExpression('0', -1, 'none', 'count', 'mm')).toBe('0')
  })

  it('U-40 an angle "45 deg" up gives "46 deg"', () => {
    expect(stepExpression('45 deg', 1, 'none', 'angle', 'mm')).toBe('46 deg')
  })

  it('steps down', () => {
    expect(stepExpression('10', -1, 'none', 'length', 'mm')).toBe('9')
    expect(stepExpression('-2.5', -1, 'none', 'length', 'mm')).toBe('-3.5')
  })

  it('steps lengths by 0.1 in centimeters, 0.01 in meters and 0.1 in inches', () => {
    expect(stepExpression('1', 1, 'none', 'length', 'cm')).toBe('1.1')
    expect(stepExpression('1', -1, 'none', 'length', 'm')).toBe('0.99')
    expect(stepExpression('1', 1, 'none', 'length', 'in')).toBe('1.1')
  })

  it('steps angles by 1 whatever the document unit', () => {
    expect(stepExpression('45', -1, 'none', 'angle', 'in')).toBe('44')
    expect(stepExpression('45deg', 1, 'coarse', 'angle', 'mm')).toBe('55deg')
  })

  it('steps scalars and counts by 1', () => {
    expect(stepExpression('0.5', 1, 'none', 'scalar', 'cm')).toBe('1.5')
    expect(stepExpression('3', 1, 'none', 'count', 'm')).toBe('4')
  })

  it('count fields ignore fine', () => {
    expect(stepExpression('3', 1, 'fine', 'count', 'mm')).toBe('4')
  })

  it('count fields never step below 0', () => {
    expect(stepExpression('3', -1, 'coarse', 'count', 'mm')).toBe('0')
  })

  it('rounds the result to 10 decimals and trims it', () => {
    expect(stepExpression('0.7', 1, 'fine', 'length', 'mm')).toBe('0.8')
    expect(stepExpression('0.2', 1, 'fine', 'length', 'cm')).toBe('0.21')
  })

  it('keeps the unit word when stepping fine', () => {
    expect(stepExpression('2.5 mm', -1, 'fine', 'length', 'mm')).toBe('2.4 mm')
  })

  it('gives null for text that is not a plain number', () => {
    expect(stepExpression('2 * 5', 1, 'none', 'length', 'mm')).toBeNull()
    expect(stepExpression('', 1, 'none', 'length', 'mm')).toBeNull()
  })
})

describe('isPlainNumber', () => {
  it.each(['10', '-2.5', '10mm', '10 mm', '45 deg'])('U-41 is true for %j', (text) => {
    expect(isPlainNumber(text)).toBe(true)
  })

  it.each(['2*5', 'wall', '10 mm + 1', ''])('U-41 is false for %j', (text) => {
    expect(isPlainNumber(text)).toBe(false)
  })

  it.each(['+3', '0.25', '10 cm', '1 m', '2 in', '1.5 rad', '45deg'])('is true for %j', (text) => {
    expect(isPlainNumber(text)).toBe(true)
  })

  it.each(['10 ft', 'mm', '1e3', '10 mm mm', '--1', 'size'])('is false for %j', (text) => {
    expect(isPlainNumber(text)).toBe(false)
  })
})
