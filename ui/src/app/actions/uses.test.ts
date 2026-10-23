import { describe, expect, it } from 'vitest'
import type { Use } from '../../generated/protocol'
import { createAppStore, INITIAL_STATE } from '../../state/store'
import { createTestApp, currentDocument } from '../../test/fakes/app'
import { createFakeCoreApi, type FakeCoreApi } from '../../test/fakes/core-api'
import { deferred } from '../../test/fakes/deferred'
import { settle } from '../../test/fakes/settle'
import { stateWithDocument } from '../../test/fakes/state'
import { DROP_DOCUMENT, DROP_USES } from '../../test/fakes/uses'
import type { App } from '../createApp'
import { refreshUses } from './timeline'

// `refreshUses` (ui.md 2.9, 3.9, U-148): `list_uses` outside the mutation queue, at most once per
// revision, and stored only while the revision it describes is still the document's.

type Harness = { readonly app: App; readonly core: FakeCoreApi }

/** U-135's document at `revision`, with `uses` known at `usesRevision` (none when null). */
function harness(
  revision = 5,
  known: { readonly revision: number; readonly uses: readonly Use[] } | null = null,
): Harness {
  const core = createFakeCoreApi({ listUses: () => Promise.resolve(DROP_USES) })
  const app = createTestApp({
    core,
    store: createAppStore(stateWithDocument(DROP_DOCUMENT, {}, { revision, uses: known })),
  })
  return { app, core }
}

/** Moves the open document to `revision`, as a change the core reported would. */
function changeRevision(app: App, revision: number): void {
  app.store.setState((state) => ({
    document: state.document === null ? null : { ...state.document, revision },
  }))
}

describe('refreshUses (3.9, U-148)', () => {
  it('U-148 asks for the uses while the document has none, and stores them with their revision', async () => {
    const { app, core } = harness()
    await refreshUses(app)
    expect(core.callsTo('listUses')).toEqual([[1]])
    expect(currentDocument(app).uses).toEqual({ revision: 5, uses: DROP_USES })
  })

  it('U-148 asks again when the uses are older than the revision', async () => {
    const { app, core } = harness(5, { revision: 4, uses: [] })
    await refreshUses(app)
    expect(core.callsTo('listUses')).toEqual([[1]])
    expect(currentDocument(app).uses).toEqual({ revision: 5, uses: DROP_USES })
  })

  it('U-148 does not ask again at the same revision', async () => {
    const { app, core } = harness()
    await refreshUses(app)
    await refreshUses(app)
    expect(core.callsTo('listUses')).toHaveLength(1)
  })

  it('U-148 asks once more after a change, and keeps the older uses until the answer arrives', async () => {
    const { app, core } = harness()
    await refreshUses(app)
    changeRevision(app, 6)
    const answer = deferred<readonly Use[]>()
    core.stub('listUses', () => answer.promise)
    const refreshed = refreshUses(app)
    await settle()
    expect(core.callsTo('listUses')).toHaveLength(2)
    expect(currentDocument(app).uses).toEqual({ revision: 5, uses: DROP_USES })
    answer.resolve([])
    await refreshed
    expect(currentDocument(app).uses).toEqual({ revision: 6, uses: [] })
  })

  it('U-148 drops an answer whose revision is older than the document when it arrives', async () => {
    const { app, core } = harness()
    const answer = deferred<readonly Use[]>()
    // A request for the newer revision, if one is made, stays unanswered.
    const later = deferred<readonly Use[]>()
    core.stub('listUses', () =>
      core.callsTo('listUses').length === 1 ? answer.promise : later.promise,
    )
    void refreshUses(app)
    await settle()
    changeRevision(app, 6)
    answer.resolve(DROP_USES)
    await settle()
    expect(currentDocument(app).uses).toBeNull()
  })

  it('U-148 sends one request for two calls at one revision while the first is in flight', async () => {
    const { app, core } = harness()
    const answer = deferred<readonly Use[]>()
    core.stub('listUses', () => answer.promise)
    const first = refreshUses(app)
    const second = refreshUses(app)
    await settle()
    expect(core.callsTo('listUses')).toHaveLength(1)
    answer.resolve(DROP_USES)
    await Promise.all([first, second])
    expect(currentDocument(app).uses).toEqual({ revision: 5, uses: DROP_USES })
  })

  it('asks outside the mutation queue: a change still waiting does not hold it back (2.6)', async () => {
    const { app, core } = harness()
    const change = deferred<undefined>()
    void app.mutations.run(() => change.promise)
    await refreshUses(app)
    expect(core.callsTo('listUses')).toEqual([[1]])
    change.resolve(undefined)
  })

  it('asks nothing without a document', async () => {
    const core = createFakeCoreApi()
    const app = createTestApp({ core, store: createAppStore(INITIAL_STATE) })
    await refreshUses(app)
    expect(core.callsTo('listUses')).toEqual([])
  })
})
