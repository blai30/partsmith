import { afterEach, describe, expect, it, vi } from 'vitest'
import { createAppStore, type AppStore, type DocumentState } from '../state/store'
import { emptyDocument } from '../test/fakes/documents'
import { settle } from '../test/fakes/settle'
import { stateWithDocument } from '../test/fakes/state'
import { createFakeApp } from '../test/fakes/app'
import { createFakeTauri, rejects, resolves, type FakeTauri } from '../test/fakes/tauri'
import { installCloseGuard } from './close-guard'
import { installUnloadGuard } from './createApp'

const WARNING = 'Partsmith could not tell the desktop host about unsaved changes.'

/** A store with an open empty document whose state has `changes`. */
function storeWithDocument(changes: Partial<DocumentState> = {}): AppStore {
  return createAppStore(stateWithDocument(emptyDocument(), {}, changes))
}

/** Changes the open document as the app does when it applies an event or a save. */
function changeDocument(store: AppStore, changes: Partial<DocumentState>): void {
  store.setState((state) => ({
    document: state.document === null ? null : { ...state.document, ...changes },
  }))
}

/** A host that accepts every set_unsaved. */
function acceptingHost(): FakeTauri {
  return createFakeTauri({ set_unsaved: [resolves(null)] })
}

/** The set_unsaved payloads the host received, in order. */
function reported(fake: FakeTauri): unknown[] {
  return fake.invocations.map((invocation) => {
    expect(invocation.command).toBe('set_unsaved')
    return invocation.payload
  })
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('installCloseGuard', () => {
  it('desktop test 48: tells the host once at install that a clean document has no unsaved changes', async () => {
    const fake = acceptingHost()

    installCloseGuard(storeWithDocument({ dirty: false }), fake.tauri)
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }])
  })

  it('desktop test 48: a document_changed that makes the document dirty reports unsaved changes', async () => {
    const fake = acceptingHost()
    const store = storeWithDocument({ dirty: false })
    installCloseGuard(store, fake.tauri)

    changeDocument(store, { dirty: true, revision: 2 })
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }, { unsaved: true }])
  })

  it('desktop test 48: a second dirty change reports nothing', async () => {
    const fake = acceptingHost()
    const store = storeWithDocument({ dirty: false })
    installCloseGuard(store, fake.tauri)
    changeDocument(store, { dirty: true, revision: 2 })

    changeDocument(store, { dirty: true, revision: 3 })
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }, { unsaved: true }])
  })

  it('desktop test 48: a successful save reports no unsaved changes', async () => {
    const fake = acceptingHost()
    const store = storeWithDocument({ dirty: false })
    installCloseGuard(store, fake.tauri)
    changeDocument(store, { dirty: true, revision: 2 })

    changeDocument(store, { dirty: false, localUnsaved: false })
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }, { unsaved: true }, { unsaved: false }])
  })

  it('desktop test 48: a document with localUnsaved and not dirty has unsaved changes', async () => {
    const fake = acceptingHost()

    installCloseGuard(storeWithDocument({ dirty: false, localUnsaved: true }), fake.tauri)
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: true }])
  })

  it('desktop test 48: a rejected invoke warns once and does not throw', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const failure = new Error('Command set_unsaved not allowed by ACL')
    const fake = createFakeTauri({ set_unsaved: [rejects(failure)] })

    expect(() => installCloseGuard(storeWithDocument(), fake.tauri)).not.toThrow()
    await settle()

    expect(warn).toHaveBeenCalledTimes(1)
    expect(warn).toHaveBeenCalledWith(WARNING, failure)
  })

  it('desktop test 48: the returned function stops further invokes', async () => {
    const fake = acceptingHost()
    const store = storeWithDocument({ dirty: false })
    const stop = installCloseGuard(store, fake.tauri)

    stop()
    changeDocument(store, { dirty: true, revision: 2 })
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }])
  })

  it('a rejected invoke is not retried until the condition changes', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const fake = createFakeTauri({
      set_unsaved: [rejects(new Error('The host is busy.')), resolves(null)],
    })
    const store = storeWithDocument({ dirty: false })
    installCloseGuard(store, fake.tauri)
    await settle()

    changeDocument(store, { revision: 2 })
    await settle()
    expect(reported(fake)).toEqual([{ unsaved: false }])

    changeDocument(store, { dirty: true, revision: 3 })
    await settle()
    expect(reported(fake)).toEqual([{ unsaved: false }, { unsaved: true }])
  })

  it('no open document has no unsaved changes', async () => {
    const fake = acceptingHost()

    installCloseGuard(createAppStore(), fake.tauri)
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: false }])
  })

  it('closing a dirty document reports no unsaved changes', async () => {
    const fake = acceptingHost()
    const store = storeWithDocument({ dirty: true })
    installCloseGuard(store, fake.tauri)

    store.setState({ document: null })
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: true }, { unsaved: false }])
  })
})

describe('installCloseGuard and the beforeunload guard', () => {
  /** Document states, null for no open document. */
  const states: readonly (Partial<DocumentState> | null)[] = [
    null,
    { dirty: false, localUnsaved: false },
    { dirty: true, localUnsaved: false },
    { dirty: false, localUnsaved: true },
    { dirty: true, localUnsaved: true },
  ]

  it.each(states)('agree on whether closing loses changes for the document %j', async (changes) => {
    const store = changes === null ? createAppStore() : storeWithDocument(changes)
    const fake = acceptingHost()
    installCloseGuard(store, fake.tauri)
    const stopUnloadGuard = installUnloadGuard(createFakeApp({ store }).app, window)
    const unload = new Event('beforeunload', { cancelable: true })

    window.dispatchEvent(unload)
    stopUnloadGuard()
    await settle()

    expect(reported(fake)).toEqual([{ unsaved: unload.defaultPrevented }])
  })
})
