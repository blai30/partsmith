// The JavaScript side of the WebAssembly C ABI bridge (protocol.md 2.10, ADR 0009).

/** The three exports plus a view of the current memory. */
export type CoreModule = {
  alloc(length: number): number
  free(pointer: number, length: number): void
  call(request: number, requestLength: number, responseLengthOut: number): number
  /** The current HEAPU8; re-read after every call. */
  memory(): Uint8Array
}

/** The parts of the instantiated Emscripten module `createPartsmithCore` resolves with. */
export type EmscriptenCoreModule = {
  _ps_alloc(length: number): number
  _ps_free(pointer: number, length: number): void
  _ps_call(request: number, requestLength: number, responseLengthOut: number): number
  /** Replaced by Emscripten whenever the memory grows. */
  readonly HEAPU8: Uint8Array
}

/** The core could not allocate memory for a request or its response. */
export class CoreOutOfMemoryError extends Error {
  override readonly name = 'CoreOutOfMemoryError'
}

const LENGTH_SLOT_SIZE = 4

/** Copies a request frame into wasm memory, calls ps_call, copies the response out and frees both blocks. */
export function callCore(core: CoreModule, request: ArrayBuffer): ArrayBuffer {
  const requestLength = request.byteLength
  const requestPointer = core.alloc(requestLength)
  if (requestPointer === 0) {
    throw new CoreOutOfMemoryError(
      `The geometry core could not allocate ${String(requestLength)} bytes for a request.`,
    )
  }
  const slotPointer = core.alloc(LENGTH_SLOT_SIZE)
  if (slotPointer === 0) {
    core.free(requestPointer, requestLength)
    throw new CoreOutOfMemoryError('The geometry core could not allocate a response length slot.')
  }
  // Memory may have grown during either allocation, so every access reads core.memory() anew.
  core.memory().set(new Uint8Array(request), requestPointer)
  const responsePointer = core.call(requestPointer, requestLength, slotPointer)
  const memory = core.memory()
  const responseLength = new DataView(
    memory.buffer,
    memory.byteOffset,
    memory.byteLength,
  ).getUint32(slotPointer, true)
  core.free(requestPointer, requestLength)
  core.free(slotPointer, LENGTH_SLOT_SIZE)
  if (responsePointer === 0) {
    throw new CoreOutOfMemoryError('The geometry core could not allocate a response.')
  }
  const response = memory.slice(responsePointer, responsePointer + responseLength).buffer
  core.free(responsePointer, responseLength)
  return response
}

/** Wraps an instantiated Emscripten module as a CoreModule. */
export function wrapEmscriptenModule(module: EmscriptenCoreModule): CoreModule {
  return {
    alloc: (length) => module._ps_alloc(length),
    free: (pointer, length) => {
      module._ps_free(pointer, length)
    },
    call: (request, requestLength, responseLengthOut) =>
      module._ps_call(request, requestLength, responseLengthOut),
    memory: () => module.HEAPU8,
  }
}
