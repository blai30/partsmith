import { describe, expect, it } from 'vitest'
import { TEST_CATALOG } from '../test/fakes/catalog'
import { FIELD_HELP, fieldHelp } from './help'

const ENTRIES = Object.entries(FIELD_HELP).flatMap(([type, paths]) =>
  Object.entries(paths).map(([path, help]) => [type, path, help] as const),
)

describe('FIELD_HELP (2.12)', () => {
  it.each(ENTRIES)(
    'U-111 %s %s names a field hint of that type in the test catalog',
    (type, path) => {
      const fields = TEST_CATALOG.byType[type]?.manifest.fields ?? []
      expect(fields.map((field) => field.path)).toContain(path)
    },
  )

  it.each(ENTRIES)('%s %s is a plain sentence that ends with a period', (_type, _path, help) => {
    expect(help).toMatch(/^[A-Z0-9].*\.$/)
    expect(help).not.toMatch(/\b(omitted|default|params|None|serde)\b/)
  })

  it('gives the extrude Operation the help of the table', () => {
    expect(fieldHelp('extrude', 'op')).toBe(
      'New body makes a separate part. Join adds to the body, Cut removes from it, and Intersect keeps only the overlap.',
    )
  })

  it('has no help for a field without an entry', () => {
    expect(fieldHelp('extrude', 'extent.expr')).toBeNull()
    expect(fieldHelp('gear', 'teeth')).toBeNull()
  })
})
