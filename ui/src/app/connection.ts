import type { CoreEvent } from '../core/events'
import type { CoreReady, Transport } from '../core/transport'
import type { DocHandle, Document } from '../generated/protocol'
import type { DocumentState } from '../state/store'
import type { App } from './createApp'

// What createApp keeps about an app beyond the App type of ui.md 2.9: the transport it starts, its
// clock, the listeners that hear core events after the store, and the settled recipe of the shown
// document. An App assembled by hand (as the spec's App type allows) has none of these: it has no
// core of its own to start or to hear from.

export type AppConnection = {
  readonly transport: Transport
  /** The app's clock, in milliseconds since the epoch. */
  readonly now: () => number
  readonly eventListeners: Set<(event: CoreEvent) => void>
  /** The settled recipe (1.3) of the document with this handle while it is shown, else null. */
  readonly settledRecipe: (documentHandle: DocHandle) => Document | null
}

const connections = new WeakMap<App, AppConnection>()

/** Records what createApp made for an app. */
export function connectApp(app: App, connection: AppConnection): void {
  connections.set(app, connection)
}

/** Starts the app's transport (3.1 step 4); rejects for an app that createApp did not make. */
export function startTransport(app: App): Promise<CoreReady> {
  const connection = connections.get(app)
  if (connection === undefined) {
    return Promise.reject(new Error('This app was not made by createApp, so it has no core.'))
  }
  return connection.transport.start()
}

/** The app's clock, in milliseconds since the epoch. */
export function nowOf(app: App): number {
  return (connections.get(app)?.now ?? Date.now)()
}

/**
 * The settled recipe of a shown document (1.3): every edit the user confirmed and none of an open
 * feature command's preview. Without a settled recipe on record, the document's own recipe.
 */
export function settledRecipeOf(app: App, document: DocumentState): Document {
  return connections.get(app)?.settledRecipe(document.documentHandle) ?? document.recipe
}

/**
 * Whether stopping the app's core ends a call it is running: the worker transport terminates its
 * worker, while the desktop core runs inside the app's process (4.4, desktop.md open issue 9).
 */
export function canStopRunningCore(app: App): boolean {
  return connections.get(app)?.transport.kind !== 'tauri'
}

/**
 * Hears every core event after it reached the store, for the parts of the UI that keep data
 * outside the store, such as the scene's mesh buffers. Returns the unsubscriber.
 */
export function onCoreEvent(app: App, listener: (event: CoreEvent) => void): () => void {
  const listeners = connections.get(app)?.eventListeners
  if (listeners === undefined) {
    return () => undefined
  }
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}
