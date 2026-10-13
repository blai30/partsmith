import { describe, expect, it } from 'vitest'
import type { Document, FeatureRecord } from '../generated/protocol'
import { activeFeatures, markerPosition } from './recipe'

function records(...ids: readonly string[]): FeatureRecord[] {
  return ids.map((id) => ({ id, type: 'box' }))
}

describe('markerPosition', () => {
  it('is the number of features without a rollback feature', () => {
    expect(markerPosition(records('f-1', 'f-2', 'f-3'), null)).toBe(3)
  })

  it('is the index of the rollback feature, the first rolled-back one', () => {
    expect(markerPosition(records('f-1', 'f-2', 'f-3'), 'f-2')).toBe(1)
    expect(markerPosition(records('f-1', 'f-2', 'f-3'), 'f-1')).toBe(0)
  })

  it('is at the end when the rollback feature is not in the timeline', () => {
    expect(markerPosition(records('f-1'), 'f-9')).toBe(1)
  })
})

function recipe(features: FeatureRecord[] | undefined, rollback?: string): Document {
  return {
    format: 'partsmith',
    format_version: 2,
    units: 'mm',
    ...(features === undefined ? {} : { features }),
    ...(rollback === undefined ? {} : { rollback }),
  }
}

describe('activeFeatures', () => {
  it('is every feature without a rollback feature', () => {
    expect(activeFeatures(recipe(records('f-1', 'f-2'))).map((record) => record.id)).toEqual([
      'f-1',
      'f-2',
    ])
  })

  it('stops before the rollback feature', () => {
    expect(
      activeFeatures(recipe(records('f-1', 'f-2', 'f-3'), 'f-2')).map((record) => record.id),
    ).toEqual(['f-1'])
  })

  it('is empty for a recipe without features', () => {
    expect(activeFeatures(recipe(undefined))).toEqual([])
  })
})
