import { act, cleanup, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { installPopoverPolyfill } from '../components/popover-polyfill.testing'
import type { Document, Use } from '../generated/protocol'
import { documentState, readyState } from '../layout/fixtures.testing'
import { createModelApp, renderWithApp, stubMatchMedia } from '../layout/test-app.testing'
import { deferred } from '../test/fakes/deferred'
import { settle } from '../test/fakes/settle'
import {
  edgeReference,
  extrudeRecord,
  filletRecord,
  sketchRecord,
  USED_BY_DOCUMENT,
  USED_BY_USES,
  usesDocument,
} from '../test/fakes/uses'

// "Used by" in a timeline item's menu (ui.md 3.9, U-136): the features that use this one, from
// `list_uses`, at most three labels and "{n} more"; choosing it selects them all.

beforeEach(installPopoverPolyfill)

beforeEach(() => {
  stubMatchMedia()
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

function renderUsedBy(
  recipe: Document = USED_BY_DOCUMENT,
  listUses: () => Promise<readonly Use[]> = () => Promise.resolve(USED_BY_USES),
) {
  const test = createModelApp({ state: readyState(documentState(recipe)) })
  test.core.stub('listUses', listUses)
  renderWithApp(test.app)
  return { ...test, user: userEvent.setup() }
}

/** Opens the context menu of a timeline item and lets its `list_uses` answer arrive. */
async function openMenu(
  user: ReturnType<typeof userEvent.setup>,
  label: string,
): Promise<HTMLElement> {
  await user.pointer({
    keys: '[MouseRight]',
    target: within(screen.getByTestId('timeline')).getByRole('button', { name: label }),
  })
  await act(async () => {
    await settle()
  })
  return screen.getByTestId('context-menu')
}

function itemNames(menu: HTMLElement): string[] {
  return within(menu)
    .getAllByRole('menuitem')
    .map((menuItem) => menuItem.textContent?.trim() ?? '')
}

describe('Used by (3.9, U-136)', () => {
  it("U-136 lists the users of 'Extrude 1' in timeline order, right after Suppress", async () => {
    const { user } = renderUsedBy()
    const menu = await openMenu(user, 'Extrude 1')
    const names = itemNames(menu)
    const usedBy = "Used by 'Fillet 1', 'Datum plane 1'"
    expect(names).toContain(usedBy)
    expect(names[names.indexOf('Suppress') + 1]).toBe(usedBy)
  })

  it('U-136 choosing it selects every user', async () => {
    const { app, user } = renderUsedBy()
    const menu = await openMenu(user, 'Extrude 1')
    await user.click(
      within(menu).getByRole('menuitem', { name: "Used by 'Fillet 1', 'Datum plane 1'" }),
    )
    expect(app.store.getState().selection.map((item) => item.key)).toEqual([
      'feature:f-fil1',
      'feature:f-dp1',
    ])
  })

  it("U-136 with five users, names three and counts the rest: \"Used by 'A', 'B', 'C' and 2 more\"", async () => {
    const names = ['A', 'B', 'C', 'D', 'E']
    const fillets = names.map((name, index) =>
      filletRecord(
        `f-fil${String(index + 1)}`,
        [edgeReference(`edge(f-ext1/end-cap|f-ext1/lateral/e-${String(index + 1)})`)],
        name,
      ),
    )
    const recipe = usesDocument([
      sketchRecord('f-sk1', []),
      extrudeRecord('f-ext1', 'f-sk1', 'new'),
      ...fillets,
    ])
    const uses: readonly Use[] = fillets.map((fillet) => ({
      user: fillet.id,
      field: '/edges/0',
      feature: 'f-ext1',
    }))
    const { user } = renderUsedBy(recipe, () => Promise.resolve(uses))
    const menu = await openMenu(user, 'Extrude 1')
    expect(itemNames(menu)).toContain("Used by 'A', 'B', 'C' and 2 more")
  })

  it('U-136 a feature that nothing uses has no Used by item', async () => {
    const { user } = renderUsedBy()
    // The uses are known: the menu of 'Extrude 1' lists its users.
    expect(itemNames(await openMenu(user, 'Extrude 1'))).toContain(
      "Used by 'Fillet 1', 'Datum plane 1'",
    )
    await user.keyboard('{Escape}')
    const menu = await openMenu(user, 'Datum plane 1')
    expect(itemNames(menu).filter((name) => name.startsWith('Used by'))).toEqual([])
  })

  it('U-136 ignores a use by a feature that is not in the document', async () => {
    const uses: readonly Use[] = [
      ...USED_BY_USES,
      { user: 'f-gone', field: '/edges/0', feature: 'f-ext1' },
    ]
    const { user } = renderUsedBy(USED_BY_DOCUMENT, () => Promise.resolve(uses))
    const menu = await openMenu(user, 'Extrude 1')
    expect(itemNames(menu)).toContain("Used by 'Fillet 1', 'Datum plane 1'")
  })

  it('opening the menu asks for the uses, and the item appears once they are known', async () => {
    const answer = deferred<readonly Use[]>()
    const { core, user } = renderUsedBy(USED_BY_DOCUMENT, () => answer.promise)
    const menu = await openMenu(user, 'Extrude 1')
    expect(core.callsTo('listUses')).toEqual([[1]])
    expect(itemNames(menu).filter((name) => name.startsWith('Used by'))).toEqual([])
    await act(async () => {
      answer.resolve(USED_BY_USES)
      await settle()
    })
    expect(itemNames(screen.getByTestId('context-menu'))).toContain(
      "Used by 'Fillet 1', 'Datum plane 1'",
    )
  })

  it('a sketch used by its extrude lists it', async () => {
    const { user } = renderUsedBy()
    const menu = await openMenu(user, 'Sketch 1')
    expect(itemNames(menu)).toContain("Used by 'Extrude 1'")
  })
})
