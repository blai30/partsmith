import { describe, expect, it } from 'vitest'
import type { Document } from '../generated/protocol'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { CUBE_DOCUMENT, emptyDocument, type FeatureRecord } from '../test/fakes/documents'
import { featureLabel, nextBodyName } from './labels'

function documentWith(
  features: readonly FeatureRecord[],
  bodyNames: readonly string[] = [],
): Document {
  return {
    ...emptyDocument(),
    bodies: bodyNames.map((name, index) => ({ id: `b-${String(index + 1)}`, name })),
    features: [...features],
  }
}

function extrude(id: string, extra: Partial<FeatureRecord> = {}): FeatureRecord {
  return { id, type: 'extrude', body: 'b-1', op: 'join', ...extra }
}

describe('featureLabel', () => {
  it('labels unnamed records "<manifest label> <n>"', () => {
    expect(featureLabel(CUBE_DOCUMENT, 'f-sk1', TEST_CATALOG)).toBe('Sketch 1')
    expect(featureLabel(CUBE_DOCUMENT, 'f-ext1', TEST_CATALOG)).toBe('Extrude 1')
  })

  it('U-20 the second unnamed extrude is "Extrude 2"', () => {
    const document = documentWith([
      extrude('f-a'),
      { id: 'f-sk1', type: 'sketch', plane: { kind: 'origin', plane: 'XY' } },
      extrude('f-b'),
    ])
    expect(featureLabel(document, 'f-b', TEST_CATALOG)).toBe('Extrude 2')
  })

  it('U-20 a named record shows its name', () => {
    const document = documentWith([extrude('f-a'), extrude('f-b', { name: 'Boss' })])
    expect(featureLabel(document, 'f-b', TEST_CATALOG)).toBe('Boss')
  })

  it('U-20 suppressed records count', () => {
    const document = documentWith([extrude('f-a', { suppressed: true }), extrude('f-b')])
    expect(featureLabel(document, 'f-b', TEST_CATALOG)).toBe('Extrude 2')
  })

  it('named records count too', () => {
    const document = documentWith([extrude('f-a', { name: 'Base' }), extrude('f-b')])
    expect(featureLabel(document, 'f-b', TEST_CATALOG)).toBe('Extrude 2')
  })

  it('U-20 an unknown type uses the type name', () => {
    const document = documentWith([{ id: 'f-w', type: 'widget' }])
    expect(featureLabel(document, 'f-w', TEST_CATALOG)).toBe('widget 1')
  })

  it('without a catalog every type is unknown and uses its type name', () => {
    expect(featureLabel(CUBE_DOCUMENT, 'f-ext1', null)).toBe('extrude 1')
  })
})

describe('nextBodyName', () => {
  it('U-20 with "Body 1" and "Body 3" is "Body 2"', () => {
    expect(nextBodyName(documentWith([], ['Body 1', 'Body 3']))).toBe('Body 2')
  })

  it('is "Body 1" without bodies', () => {
    expect(nextBodyName(emptyDocument())).toBe('Body 1')
  })

  it('skips every number in use and ignores other names', () => {
    expect(nextBodyName(documentWith([], ['Body 1', 'Cube', 'Body 2']))).toBe('Body 3')
  })
})
