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
  // Performance tests (ui.md 6.7, PARTSMITH_PERF=1) and visual tests (6.6, PARTSMITH_VISUAL=1) run
  // only where they are turned on: on the fixed hardware runner (delivery.md 1.2).
  grepInvert: [
    ...(process.env.PARTSMITH_PERF === '1' ? [] : [/@perf/]),
    ...(process.env.PARTSMITH_VISUAL === '1' ? [] : [/@visual/]),
  ],
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : 'list',
  expect: {
    // Visual tests (ui.md 6.6). The per-pixel color threshold is stricter than Playwright's 0.2, which
    // passes a missing 12 percent region fill, a 30 percent problem halo or a recolored 1-pixel edge.
    toHaveScreenshot: { animations: 'disabled', maxDiffPixelRatio: 0.002, threshold: 0.05 },
  },
  use: {
    baseURL: `http://localhost:${String(port)}`,
    trace: 'retain-on-failure',
  },
  // Every project has 1280 by 720 at device scale factor 1 (ui.md 6). Chromium runs every test; Firefox and WebKit
  // run the tests tagged @engines, and only when --project names them, so pnpm e2e needs Chromium alone (ui.md 6.8).
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // Lets local runs reuse a preinstalled Chromium instead of downloading one.
        launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
          ? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
          : {},
      },
    },
    {
      name: 'firefox',
      // Headless Firefox gave no WebGL2 on the runner, so it runs headed (ui.md 6.8).
      use: { ...devices['Desktop Firefox'], headless: false },
      grep: /@engines/,
      default: false,
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'], deviceScaleFactor: 1 },
      grep: /@engines/,
      default: false,
    },
  ],
  // The production build, served with the headers of public/_headers by vite.config.ts.
  webServer: {
    command: `pnpm build && pnpm preview --port ${String(port)} --strictPort`,
    port,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
})
