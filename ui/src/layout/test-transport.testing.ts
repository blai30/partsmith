// Test support for the ui-model slice: a Transport that answers request frames from a script, so
// `createApp` and `startCore` run end to end without a core. Frames go through the real codec.
// Nothing here is part of the app.

import { decodeFrame, encodeFrame } from '../core/frame'
import type { CoreReady, Transport } from '../core/transport'
import type { Command, FrameHeader } from '../generated/protocol'
import { PROTOCOL_VERSION } from '../generated/protocol'
import { GENERIC_FDM, TEST_CATALOG_ENTRIES } from './fixtures.testing'

type JsonRecord = Readonly<Record<string, unknown>>

/** The answer to one command: an ok result (without `kind` and `type`) or an error, plus the events emitted before it. */
export type ScriptedReply =
  | { readonly result: JsonRecord; readonly events?: readonly JsonRecord[] }
  | { readonly error: { readonly code: string; readonly message: string }; readonly events?: readonly JsonRecord[] }

export type ScriptedCore = (command: Command) => ScriptedReply

/** An empty mm document as `document_changed` carries it. */
export const EMPTY_DOCUMENT: JsonRecord = {
  format: 'partsmith',
  format_version: 1,
  units: 'mm',
  parameters: [],
  bodies: [],
  features: [],
}

/** Answers the commands `startCore` sends like a healthy core with the test catalog. */
export function healthyCore(overrides: Partial<Record<Command['type'], ScriptedReply>> = {}): ScriptedCore {
  return (command) => {
    const override = overrides[command.type]
    if (override !== undefined) {
      return override
    }
    switch (command.type) {
      case 'feature_catalog':
        return { result: { features: TEST_CATALOG_ENTRIES } }
      case 'list_printer_profiles':
        return { result: { profiles: [GENERIC_FDM] } }
      case 'new_document':
        return {
          result: { doc: 1 },
          events: [
            {
              type: 'document_changed',
              doc: 1,
              revision: 0,
              document: EMPTY_DOCUMENT,
              dirty: false,
              transaction_open: false,
            },
            { type: 'recomputed', doc: 1, revision: 0, features: [], parameters: [], diagnostics: [] },
          ],
        }
      default:
        return { error: { code: 'internal_error', message: `The scripted core does not answer ${command.type}.` } }
    }
  }
}

export type ScriptedTransport = Transport & {
  /** Every command received, in order. */
  readonly commands: readonly Command[]
}

/** A worker-kind transport whose core is the script. */
export function createScriptedTransport(script: ScriptedCore): ScriptedTransport {
  const commands: Command[] = []
  const ready: CoreReady = { protocolVersion: PROTOCOL_VERSION, engineVersion: '0.1.0' }
  return {
    kind: 'worker',
    commands,
    start: () => Promise.resolve(ready),
    send(request) {
      const { header } = decodeFrame(request)
      const message = header.messages[0] as { readonly kind: string; readonly id: number; readonly command: Command }
      commands.push(message.command)
      const reply = script(message.command)
      const outcome =
        'error' in reply
          ? { kind: 'error', ...reply.error }
          : { kind: 'ok', type: message.command.type, ...reply.result }
      const messages = [
        ...(reply.events ?? []).map((event) => ({ kind: 'event', ...event })),
        { kind: 'response', id: message.id, result: outcome },
      ]
      const response = { protocol_version: PROTOCOL_VERSION, messages } as unknown as Omit<FrameHeader, 'buffers'>
      return Promise.resolve(encodeFrame(response, []))
    },
    stop: () => undefined,
    onFatal: () => () => undefined,
  }
}
