import { defineConfig, devices } from '@playwright/test'

// Temporary probe configuration of the fixer pass; deleted before the pass ends.
export default defineConfig({
  testDir: 'e2e',
  testMatch: /.*\.spec\.ts/,
  outputDir: 'test-results/fixer-out',
  fullyParallel: true,
  workers: 4,
  grepInvert: process.env.PARTSMITH_VISUAL === '1' ? [/@perf/] : [/@visual/, /@perf/],
  testIgnore: process.env.FIXER_PROBE === '1' ? [] : [/fixer-probe/],
  expect: { toHaveScreenshot: { animations: 'disabled', maxDiffPixelRatio: 0.002, threshold: 0.05 } },
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:4175',
    launchOptions: { executablePath: '/opt/pw-browsers/chromium' },
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
