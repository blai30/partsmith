import type { Command, Request } from '../generated/protocol'
import { errorResponseFrame } from './frame'

// Request superseding (protocol.md 2.11 rule 4), shared by the worker host and the Tauri transport.

/** The message of a synthesized `superseded` response (protocol.md section 4). */
export const SUPERSEDED_MESSAGE = 'A newer request replaced this one.'

function documentOf(command: Command): number | null {
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
  const document = documentOf(command)
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
    if (documentOf(candidate) !== document) {
      continue
    }
    // The first later request for the same document decides: the same kind for the same sketch
    // replaces this one; anything else must see this one's effect first.
    return candidate.type === command.type && sketchOf(candidate) === sketch
  }
  return false
}

/** The response frame that answers a superseded request. */
export function supersededFrame(id: number): ArrayBuffer {
  return errorResponseFrame(id, 'superseded', SUPERSEDED_MESSAGE)
}
