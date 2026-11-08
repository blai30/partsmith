import { describe, expect, it } from 'vitest'
import { TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'

// Plan item 6 moved the UI's tables of feature data into the manifests (ui.md 2.10 and 2.12, ADR
// 0065): no module keeps a description, a help text, a start value or the body-creating rule of
// its own (U-285).

/** The text of every `.ts` and `.tsx` file under `ui/src` but test files and `src/test/`, by path. */
const SOURCES = import.meta.glob<string>(
  ['/src/**/*.{ts,tsx}', '!/src/**/*.test.{ts,tsx}', '!/src/test/**'],
  { query: '?raw', import: 'default', eager: true },
)

/** The modules that held the tables in M1. */
const REMOVED_FILES = [
  '/src/commands/descriptions.ts',
  '/src/inspector/help.ts',
  '/src/inspector/start-values.ts',
]

/** The identifiers of the tables and the inference they held. */
const REMOVED_IDENTIFIERS = [
  'FEATURE_DESCRIPTIONS',
  'FIELD_HELP',
  'START_VALUES',
  'HEAD_DIAMETER',
  'bodyCreatingTarget',
]

/** Every description and help text of the test catalog, each once. */
const MANIFEST_TEXTS = [
  ...new Set(
    TEST_CATALOG_ENTRIES.flatMap((entry) => [
      entry.manifest.description,
      ...entry.manifest.fields.flatMap((hint) => hint.help ?? []),
    ]),
  ),
]

/** The paths of the files that contain `text`. */
function filesContaining(text: string): readonly string[] {
  return Object.entries(SOURCES)
    .filter(([, source]) => source.includes(text))
    .map(([path]) => path)
}

describe("none of plan item 6's UI-only tables remains (U-285)", () => {
  it('U-285 removes descriptions.ts, help.ts and start-values.ts', () => {
    for (const path of REMOVED_FILES) {
      expect(Object.keys(SOURCES)).not.toContain(path)
    }
  })

  it.each(REMOVED_IDENTIFIERS)('U-285 leaves no module that names %s', (identifier) => {
    expect(filesContaining(identifier)).toEqual([])
  })

  it.each(MANIFEST_TEXTS)('U-285 leaves no module that holds the text "%s"', (text) => {
    expect(filesContaining(text)).toEqual([])
  })

  it('reads the modules, without tests or test support', () => {
    const paths = Object.keys(SOURCES)
    expect(paths).toContain('/src/commands/features.ts')
    expect(paths).toContain('/src/inspector/model.ts')
    expect(paths.some((path) => path.endsWith('.test.ts') || path.startsWith('/src/test/'))).toBe(
      false,
    )
  })

  it('checks every description of the test catalog and some help', () => {
    expect(MANIFEST_TEXTS.length).toBeGreaterThan(TEST_CATALOG_ENTRIES.length)
    expect(MANIFEST_TEXTS.every((text) => text.length > 0)).toBe(true)
  })
})
