import { existsSync } from 'node:fs'
import { defineConfig, devices } from '@playwright/test'

const port = 4173

// The tests run against the real WebAssembly core that scripts/build-wasm.sh copies into public/core (ui.md 6).
const missingCoreFiles = ['partsmith_core.mjs', 'partsmith_core.wasm'].filter(
  (name) => !existsSync(new URL(`public/core/${name}`, import.meta.url)),
)
if (missingCoreFiles.length > 0) {
  throw new Error(
    `The end-to-end tests need the web core in ui/public/core, which lacks ${missingCoreFiles.join(' and ')}. Build it with scripts/build-wasm.sh (see ui/e2e/README.md).`,
  )
}

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: 0,
  // Performance tests run only on the fixed hardware runner, with PARTSMITH_PERF=1 (ui.md 6.7).
  grepInvert: process.env.PARTSMITH_PERF === '1' ? undefined : /@perf/,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
  expect: {
    // Visual tests (ui.md 6.6). The per-pixel color threshold is stricter than Playwright's 0.2, which
    // passes a missing 12 percent region fill, a 30 percent problem halo or a recolored 1-pixel edge.
    toHaveScreenshot: { animations: 'disabled', maxDiffPixelRatio: 0.002, threshold: 0.05 },
  },
  use: {
    baseURL: `http://localhost:${String(port)}`,
    trace: 'retain-on-failure',
    // Lets local runs reuse a preinstalled Chromium instead of downloading one.
    launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
      : {},
  },
  // Desktop Chrome: 1280 by 720 at device scale factor 1 (ui.md 6).
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  // The production build, served with the cross-origin isolation headers from vite.config.ts.
  webServer: {
    command: `pnpm build && pnpm preview --port ${String(port)} --strictPort`,
    port,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
})
