import { defineConfig, devices } from '@playwright/test'

// Temporary probe configuration of the fixer pass; deleted before the pass ends.
export default defineConfig({
  testDir: 'e2e',
  testMatch: 'fixer-probe.spec.ts',
  outputDir: 'test-results/fixer-out',
  fullyParallel: true,
  workers: 4,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:4175',
    launchOptions: { executablePath: '/opt/pw-browsers/chromium' },
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
