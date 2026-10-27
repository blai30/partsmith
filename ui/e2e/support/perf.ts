import { expect, test } from '@playwright/test'

// The reporting of the performance tests (ui.md 6.7), shared by perf.spec.ts and the engine's web-column rows in
// engine-budgets.spec.ts (engine.md section 5).

/** A measured value and the budget it is held to. */
export type Measured = { readonly statistic: string; readonly value: number; readonly budget: number }

/** A value in milliseconds rounded to 0.01 ms, as the annotations write it. */
function formatMilliseconds(value: number): string {
  return String(Math.round(value * 100) / 100)
}

/**
 * Adds one `perf` annotation per measured value, written `{id} {statistic}: {value} ms (budget
 * {budget} ms)`, then holds every value to its budget (ui.md 6.7).
 */
export function expectWithinBudgets(id: string, measured: readonly Measured[]): void {
  for (const { statistic, value, budget } of measured) {
    test.info().annotations.push({
      type: 'perf',
      description: `${id} ${statistic}: ${formatMilliseconds(value)} ms (budget ${String(budget)} ms)`,
    })
  }
  for (const { statistic, value, budget } of measured) {
    expect.soft(value, `${id} ${statistic}`).toBeLessThanOrEqual(budget)
  }
}
