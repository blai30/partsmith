import { afterEach, describe, expect, it, vi } from 'vitest'
import { startCore } from '../app/actions/document'
import type { CatalogEntry } from '../generated/protocol'
import { registerMissingCommands } from '../layout/test-app.testing'
import { createFakeApp } from '../test/fakes/app'
import { EXTRUDE_ENTRY, TEST_CATALOG_ENTRIES } from '../test/fakes/catalog'
import { settle } from '../test/fakes/settle'

afterEach(() => {
  vi.restoreAllMocks()
})

function brokenEntry(params_schema: unknown): CatalogEntry {
  return {
    type: 'broken',
    manifest: { label: 'Broken', category: 'create', icon: 'broken', version: 1, fields: [] },
    params_schema: params_schema as CatalogEntry['params_schema'],
  }
}

async function startWithCatalog(catalog: readonly CatalogEntry[]) {
  const fake = createFakeApp({ core: { catalog } })
  await startCore(fake.app)
  await settle()
  return fake.app
}

describe('the catalog store (2.12)', () => {
  it('U-30 maps feature_catalog entries to the Catalog in protocol order, keyed by type', async () => {
    const app = await startWithCatalog(TEST_CATALOG_ENTRIES)
    const catalog = app.store.getState().catalog
    expect(catalog?.features.map((feature) => feature.type)).toEqual(
      TEST_CATALOG_ENTRIES.map((entry) => entry.type),
    )
    expect(Object.keys(catalog?.byType ?? {}).toSorted()).toEqual(
      TEST_CATALOG_ENTRIES.map((entry) => entry.type).toSorted(),
    )
    expect(catalog?.byType.extrude).toBe(catalog?.features.find((feature) => feature.type === 'extrude'))
  })

  it('U-30 passes manifests and params schemas through unchanged', async () => {
    const app = await startWithCatalog(TEST_CATALOG_ENTRIES)
    const extrude = app.store.getState().catalog?.byType.extrude
    expect(extrude?.manifest).toEqual(EXTRUDE_ENTRY.manifest)
    expect(extrude?.paramsSchema).toEqual(EXTRUDE_ENTRY.params_schema)
  })

  it('U-30 leaves out an entry whose params_schema is not an object, and reports it once', async () => {
    const report = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const app = await startWithCatalog([...TEST_CATALOG_ENTRIES, brokenEntry(true)])
    const catalog = app.store.getState().catalog
    expect(catalog?.byType.broken).toBeUndefined()
    expect(catalog?.features).toHaveLength(TEST_CATALOG_ENTRIES.length)
    expect(report).toHaveBeenCalledTimes(1)
  })

  it('leaves out an entry whose params_schema is not an object schema', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const app = await startWithCatalog([...TEST_CATALOG_ENTRIES, brokenEntry({ type: 'string' })])
    expect(app.store.getState().catalog?.byType.broken).toBeUndefined()
  })

  it('registers a command for each catalog type once the core has started (3.1)', async () => {
    const app = await startWithCatalog(TEST_CATALOG_ENTRIES)
    expect(app.commands.get('feature.extrude')?.title).toBe('Extrude')
    expect(app.commands.get('feature.import')).toBeUndefined()
    // Registering what is missing adds nothing that startCore already registered.
    expect(() => {
      registerMissingCommands(app)
    }).not.toThrow()
  })
})
