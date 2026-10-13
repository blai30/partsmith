import { featureCommands } from '../commands/features'
import type { CatalogEntry } from '../generated/protocol'
import { adoptDocument } from '../state/core-events'
import type { AppStore, Catalog, CatalogFeature } from '../state/store'
import type { App } from './createApp'

// Starting a core (ui.md 3.1 step 4, 4.4): the session data every started core provides (the
// feature catalog and the printer profiles), the core's status, and the first document.

function isObjectSchema(schema: unknown): schema is Readonly<Record<string, unknown>> {
  return (
    typeof schema === 'object' &&
    schema !== null &&
    !Array.isArray(schema) &&
    Reflect.get(schema, 'type') === 'object'
  )
}

/** The store's catalog for protocol entries; an entry without an object params schema is left out and reported (2.12). */
export function catalogFromEntries(entries: readonly CatalogEntry[]): Catalog {
  const features: CatalogFeature[] = []
  for (const entry of entries) {
    const schema: unknown = entry.params_schema
    if (!isObjectSchema(schema)) {
      console.error(
        `The feature type ${entry.type} has no object params schema, so Partsmith leaves it out.`,
      )
      continue
    }
    features.push({ type: entry.type, manifest: entry.manifest, paramsSchema: schema })
  }
  return {
    features,
    byType: Object.fromEntries(features.map((feature) => [feature.type, feature])),
  }
}

/** Loads the catalog and printers into the store and registers a command per new feature type. */
export async function loadCoreData(app: App): Promise<void> {
  const [entries, printers] = await Promise.all([
    app.core.featureCatalog(),
    app.core.listPrinterProfiles(),
  ])
  const catalog = catalogFromEntries(entries)
  app.store.setState({ catalog, printers })
  for (const command of featureCommands(catalog)) {
    if (app.commands.get(command.id) === undefined) {
      app.commands.register(command)
    }
  }
}

/** Marks the core ready with the engine version it reported. */
export function markCoreReady(store: AppStore, engineVersion: string): void {
  store.setState((state) => ({
    core: { ...state.core, status: 'ready', engineVersion, fatal: null },
  }))
}

/** Marks the core failed with the message its fatal state shows (4.4). */
export function markCoreFailed(store: AppStore, message: string): void {
  store.setState((state) => ({ core: { ...state.core, status: 'failed', fatal: message } }))
}

/** Opens a new empty mm document named "Untitled". */
export async function openUntitled(app: App): Promise<void> {
  const documentHandle = await app.mutations.run(() => app.core.newDocument('mm'), 'other')
  adoptDocument(app.store, documentHandle, { name: 'Untitled', file: null, localUnsaved: false })
}
