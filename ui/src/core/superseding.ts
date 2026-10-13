import type { Command, Request } from '../generated/protocol'
import { decodeFrame, errorResponseFrame } from './frame'

// Request superseding (protocol.md 2.11 rule 4), shared by the worker host and the Tauri transport.

/** The message of a synthesized `superseded` response (protocol.md section 4). */
export const SUPERSEDED_MESSAGE = 'A newer request replaced this one.'

function documentHandleOf(command: Command): number | null {
  return 'doc' in command ? command.doc : null
}

function sketchOf(command: Command): string | null {
  return command.type === 'drag_sketch' || command.type === 'solve_sketch' ? command.feature : null
}

/** Whether the queued request at `index` is superseded by a later request in `queue` (rule 4). `queue` holds requests that have not started, oldest first. */
export function isSuperseded(queue: readonly Request[], index: number): boolean {
  const command = queue[index]?.command
  if (command === undefined) {
    return false
  }
  const document = documentHandleOf(command)
  const later = queue.slice(index + 1).map((request) => request.command)
  if (command.type === 'analyze_print') {
    return later.some(
      (candidate) => candidate.type === 'analyze_print' && candidate.doc === document,
    )
  }
  const sketch = sketchOf(command)
  if (sketch === null) {
    return false
  }
  for (const candidate of later) {
    if (documentHandleOf(candidate) !== document) {
      continue
    }
    // The first later request for the same document decides: the same kind for the same sketch
    // replaces this one; anything else must see this one's effect first.
    return candidate.type === command.type && sketchOf(candidate) === sketch
  }
  return false
}

/** The request a queued frame carries, or null when the frame does not decode to one request. */
export function requestOf(frame: ArrayBuffer): Request | null {
  try {
    const [message] = decodeFrame(frame).header.messages
    return message?.kind === 'request' ? { id: message.id, command: message.command } : null
  } catch {
    // The core answers an unreadable frame itself (malformed_frame).
    return null
  }
}

/**
 * Whether the call at the head of the queue is superseded. Superseding looks only as far as the
 * first unreadable frame, so a request the host cannot read never lets a later one overtake it.
 */
export function headIsSuperseded(queue: readonly { readonly request: Request | null }[]): boolean {
  const readable: Request[] = []
  for (const call of queue) {
    if (call.request === null) {
      break
    }
    readable.push(call.request)
  }
  return readable.length > 0 && isSuperseded(readable, 0)
}

/** The response frame that answers a superseded request. */
export function supersededFrame(id: number): ArrayBuffer {
  return errorResponseFrame(id, 'superseded', SUPERSEDED_MESSAGE)
}
