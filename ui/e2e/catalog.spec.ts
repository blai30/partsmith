import { expect } from '@playwright/test'
import { TEST_CATALOG_ENTRIES } from '../src/test/fakes/catalog'
import { test } from './helpers'
import { ok, openEngine } from './support/engine-budgets'
import { serveTestPage } from './support/test-pages'

// The test catalog is the web core's (ui.md E-174, ADR 0065): the manifests the Vitest tests read
// from src/test/fakes/catalog.ts are the ones the web core sends, the description and each hint's
// help, start value and new body included. Chromium only, so not tagged @engines.

const enginePage = serveTestPage('engine.html')

test("E-174 the test catalog's manifests are the web core's", async ({ page }) => {
  const engine = await openEngine(page, enginePage.url)
  const instance = await engine.start()

  const result = ok(await engine.call(instance, { type: 'feature_catalog' }))

  if (result.type !== 'feature_catalog') {
    throw new Error(`feature_catalog answered ${result.type}.`)
  }
  expect(result.features.map((entry) => entry.type)).toEqual(
    TEST_CATALOG_ENTRIES.map((entry) => entry.type),
  )
  for (const entry of TEST_CATALOG_ENTRIES) {
    const sent = result.features.find((candidate) => candidate.type === entry.type)
    expect(sent?.manifest, `the manifest of ${entry.type}`).toEqual(entry.manifest)
  }
})
