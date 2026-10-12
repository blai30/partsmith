import { useEffect, useRef } from 'react'
import type { App } from '../app/createApp'

// Requests from commands to the chrome that shows them (ui.md 3.4, 3.9, 3.10, 3.18): a command run
// from the palette or a shortcut asks the component that owns a popover, a menu or an inline editor
// to open it. A request no component is mounted to take waits until one mounts.

export type UiRequest =
  /** Open the export popover (3.18). */
  | { readonly kind: 'export' }
  /** Open the printer menu of the Print toolbar (3.17). */
  | { readonly kind: 'printer' }
  /** Open the checks popover of the Print toolbar (3.17). */
  | { readonly kind: 'checks' }
  /** Rename a feature or body inline, in the timeline or the browser (3.9, 3.10). */
  | { readonly kind: 'rename'; readonly key: string; readonly where: 'timeline' | 'browser' }
  /** Focus a parameter's expression in the Parameters panel (3.4). */
  | { readonly kind: 'parameter'; readonly id: string }

export type UiRequestKind = UiRequest['kind']

type RequestOf<Kind extends UiRequestKind> = Extract<UiRequest, { readonly kind: Kind }>

/** Takes a request; returns false to leave it to another listener or to the next one that mounts. */
type Listener = (request: UiRequest) => boolean

type Channel = {
  readonly listeners: Set<Listener>
  /** The latest request of each kind that no listener took. */
  readonly waiting: Map<UiRequestKind, UiRequest>
}

const channels = new WeakMap<App, Channel>()

function channelOf(app: App): Channel {
  let channel = channels.get(app)
  if (channel === undefined) {
    channel = { listeners: new Set(), waiting: new Map() }
    channels.set(app, channel)
  }
  return channel
}

/** Asks the chrome to show something; it waits for a component when none takes it now. */
export function requestUi(app: App, request: UiRequest): void {
  const channel = channelOf(app)
  for (const listener of Array.from(channel.listeners)) {
    if (listener(request)) {
      channel.waiting.delete(request.kind)
      return
    }
  }
  channel.waiting.set(request.kind, request)
}

/** Listens for requests; returns the unsubscriber. A waiting request is offered at once. */
export function onUiRequest(app: App, listener: Listener): () => void {
  const channel = channelOf(app)
  channel.listeners.add(listener)
  for (const [kind, request] of Array.from(channel.waiting)) {
    if (listener(request)) {
      channel.waiting.delete(kind)
    }
  }
  return () => {
    channel.listeners.delete(listener)
  }
}

/**
 * Handles requests of one kind while the component is mounted. The handler returns false to
 * decline a request (for example a rename of an item it does not show).
 */
export function useUiRequest<Kind extends UiRequestKind>(
  app: App,
  kind: Kind,
  handler: (request: RequestOf<Kind>) => boolean,
): void {
  const latest = useRef(handler)
  useEffect(() => {
    latest.current = handler
  })
  useEffect(
    () =>
      onUiRequest(app, (request) =>
        request.kind === kind ? latest.current(request as RequestOf<Kind>) : false,
      ),
    [app, kind],
  )
}
