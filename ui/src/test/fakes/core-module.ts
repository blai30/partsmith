import type { CoreModule } from '../../core/bridge'

// A CoreModule (protocol.md 2.10) over an in-memory frame handler: ps_alloc, ps_free and ps_call on a
// linear memory, with the WebAssembly memory growth the bridge must survive. Every ps_call replaces
// the memory's ArrayBuffer by a larger copy before writing the response, as Emscripten's
// ALLOW_MEMORY_GROWTH does, so a caller that keeps an old memory() view reads stale bytes.

/** Answers one request frame with a response frame. */
export type FrameHandler = (request: ArrayBuffer) => ArrayBuffer

/** A block of the fake memory. */
export type Block = { readonly pointer: number; readonly length: number }

export type FakeCoreModule = CoreModule & {
  /** Every block handed out, by `alloc` or for a response, in order. */
  readonly allocations: readonly Block[]
  /** Every `free` call, in order. */
  readonly frees: readonly Block[]
  /** The request frames passed to `call`, copied. */
  readonly requests: readonly ArrayBuffer[]
  /** Blocks handed out and not freed, by pointer. */
  readonly live: ReadonlyMap<number, number>
  /** When true, `alloc` returns 0 (allocation failure). */
  failAllocation: boolean
  /** When true, `call` returns 0 and writes 0 (the response could not be allocated). */
  failResponse: boolean
  /** When set, `call` throws it (a trap). */
  callError: Error | null
}

const PAGE = 65_536

export function createFakeCoreModule(handler: FrameHandler): FakeCoreModule {
  let memory = new ArrayBuffer(PAGE)
  let next = 8
  const allocations: Block[] = []
  const frees: Block[] = []
  const requests: ArrayBuffer[] = []
  const live = new Map<number, number>()

  function grow(minimum: number): void {
    const size = Math.ceil((Math.max(minimum, memory.byteLength) + PAGE) / PAGE) * PAGE
    const larger = new ArrayBuffer(size)
    new Uint8Array(larger).set(new Uint8Array(memory))
    memory = larger
  }

  function allocate(length: number): number {
    if (length === 0) {
      return 0
    }
    const pointer = next
    next = Math.ceil((pointer + length) / 8) * 8
    if (next > memory.byteLength) {
      grow(next)
    }
    allocations.push({ pointer, length })
    live.set(pointer, length)
    return pointer
  }

  const module: FakeCoreModule = {
    allocations,
    frees,
    requests,
    live,
    failAllocation: false,
    failResponse: false,
    callError: null,
    alloc(length) {
      return module.failAllocation ? 0 : allocate(length)
    },
    free(pointer, length) {
      frees.push({ pointer, length })
      if (pointer === 0) {
        return
      }
      if (live.get(pointer) !== length) {
        throw new Error(`free(${String(pointer)}, ${String(length)}) does not match a live block.`)
      }
      live.delete(pointer)
    },
    call(request, requestLength, responseLengthOut) {
      if (module.callError !== null) {
        throw module.callError
      }
      const frame = new Uint8Array(memory, request, requestLength).slice().buffer
      requests.push(frame)
      const response = new Uint8Array(handler(frame))
      grow(memory.byteLength)
      const view = new DataView(memory)
      if (module.failResponse) {
        view.setUint32(responseLengthOut, 0, true)
        return 0
      }
      const pointer = allocate(response.length)
      new Uint8Array(memory).set(response, pointer)
      new DataView(memory).setUint32(responseLengthOut, response.length, true)
      return pointer
    },
    memory() {
      return new Uint8Array(memory)
    },
  }
  return module
}
