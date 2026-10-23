import { describe, expect, it } from 'vitest'
import { assetName } from '../test/fakes/assets'
import { ASSET_NAME, referencedAssetNames } from './assets'

// The asset names a document's text references, which Open reads from its assets folder (ui.md
// 2.15, 3.19 step 3, ADR 0037 decision 2).

const A = assetName('a')
const B = assetName('b')

describe('referencedAssetNames (2.15)', () => {
  it('U-217 collects each asset name among the string values once, sorted', () => {
    const text = JSON.stringify({
      features: [{ asset: B }, { asset: A, name: A }],
      [A]: 1,
      list: [`${'a'.repeat(64)}.STEP`, 'x.step', `${'a'.repeat(63)}.step`, 7],
    })
    expect(referencedAssetNames(text)).toEqual([A, B])
  })

  it('U-217 a text that is not JSON names no asset', () => {
    expect(referencedAssetNames('not json')).toEqual([])
  })

  it('U-217 an object key that is an asset name is not a value', () => {
    expect(referencedAssetNames(JSON.stringify({ [A]: { [B]: true } }))).toEqual([])
  })

  it('finds names at any depth and in the nested lists of a document', () => {
    const text = JSON.stringify({ a: { b: [[{ c: A }]] }, d: [B, B] })
    expect(referencedAssetNames(text)).toEqual([A, B])
  })

  it('a JSON text that is a single string names that asset', () => {
    expect(referencedAssetNames(JSON.stringify(A))).toEqual([A])
  })
})

describe('ASSET_NAME (2.15)', () => {
  it('U-217 matches an asset name and not one followed by a line feed', () => {
    expect(ASSET_NAME.test(A)).toBe(true)
    expect(ASSET_NAME.test(`${A}\n`)).toBe(false)
  })

  it('is the pattern of python-api.md 3.1: 64 lowercase hex digits, a dot, a lowercase extension', () => {
    expect(ASSET_NAME.source).toBe('^[0-9a-f]{64}\\.[a-z0-9]+$')
    expect(ASSET_NAME.test(`${'0'.repeat(64)}.3mf`)).toBe(true)
    expect(ASSET_NAME.test(`${'0'.repeat(64)}.`)).toBe(false)
    expect(ASSET_NAME.test(`${'0'.repeat(63)}A.step`)).toBe(false)
    expect(ASSET_NAME.test(`../${A}`)).toBe(false)
  })
})
