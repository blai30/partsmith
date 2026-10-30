import { fireEvent } from '@testing-library/react'
import { afterEach, describe, expect, it, vi, type Mock } from 'vitest'
import { rejectionOf } from '../test/fakes/deferred'
import { utf8 } from '../test/fakes/documents'
import {
  createWebFileService,
  type DesktopFileHandle,
  FILE_TYPES,
  FileServiceError,
  isFileHandle,
} from './file-service'

const CONTENT = '{"format": 1}'
const BYTES = utf8(CONTENT)
const PTSM_PICKER_TYPE = {
  description: 'Partsmith model',
  accept: { 'application/json': ['.ptsm'] },
}

type PermissionMode = { readonly mode: 'read' | 'readwrite' }

type FakeHandleOptions = {
  readonly name?: string
  readonly permission?: PermissionState
  readonly requestResult?: PermissionState
  readonly getFileError?: Error
  readonly createWritableError?: Error
  readonly writeError?: Error
}

type FakeHandle = {
  readonly handle: FileSystemFileHandle
  readonly queryPermission: Mock<(descriptor: PermissionMode) => Promise<PermissionState>>
  readonly requestPermission: Mock<(descriptor: PermissionMode) => Promise<PermissionState>>
  readonly getFile: Mock<() => Promise<File>>
  readonly createWritable: Mock<() => Promise<unknown>>
  readonly write: Mock<(data: unknown) => Promise<void>>
  readonly close: Mock<() => Promise<void>>
}

// A file handle as Chromium's File System Access API provides it, with the permission methods.
function fakeHandle(options: FakeHandleOptions = {}): FakeHandle {
  const name = options.name ?? 'bracket.ptsm'
  const write = vi.fn<(data: unknown) => Promise<void>>(async () => {
    if (options.writeError !== undefined) {
      throw options.writeError
    }
  })
  const close = vi.fn<() => Promise<void>>(async () => undefined)
  const queryPermission = vi.fn<(descriptor: PermissionMode) => Promise<PermissionState>>(
    async () => options.permission ?? 'granted',
  )
  const requestPermission = vi.fn<(descriptor: PermissionMode) => Promise<PermissionState>>(
    async () => options.requestResult ?? 'granted',
  )
  const getFile = vi.fn<() => Promise<File>>(async () => {
    if (options.getFileError !== undefined) {
      throw options.getFileError
    }
    return new File([CONTENT], name, { type: 'application/json' })
  })
  const createWritable = vi.fn<() => Promise<unknown>>(async () => {
    if (options.createWritableError !== undefined) {
      throw options.createWritableError
    }
    return { write, close }
  })
  const handle = {
    kind: 'file',
    name,
    queryPermission,
    requestPermission,
    getFile,
    createWritable,
    isSameEntry: async (other: unknown) => other === handle,
  }
  return {
    handle: handle as unknown as FileSystemFileHandle,
    queryPermission,
    requestPermission,
    getFile,
    createWritable,
    write,
    close,
  }
}

function abortError(): DOMException {
  return new DOMException('The user aborted a request.', 'AbortError')
}

function notFoundError(): DOMException {
  return new DOMException('A requested file or directory could not be found.', 'NotFoundError')
}

// A picker these tests never call.
function unusedPicker(): Mock<() => Promise<never>> {
  return vi.fn<() => Promise<never>>(async () => {
    throw new Error('This picker is not expected to be called.')
  })
}

function provideFileSystemAccess(open: Mock, save: Mock): void {
  Object.defineProperty(window, 'showOpenFilePicker', {
    value: open,
    configurable: true,
    writable: true,
  })
  Object.defineProperty(window, 'showSaveFilePicker', {
    value: save,
    configurable: true,
    writable: true,
  })
}

const ORIGINAL_CREATE_OBJECT_URL = Object.getOwnPropertyDescriptor(URL, 'createObjectURL')
const ORIGINAL_REVOKE_OBJECT_URL = Object.getOwnPropertyDescriptor(URL, 'revokeObjectURL')

function stubObjectUrls(): {
  readonly createObjectURL: Mock<(blob: Blob) => string>
  readonly revokeObjectURL: Mock<(url: string) => void>
} {
  const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:http://localhost/3f2c')
  const revokeObjectURL = vi.fn<(url: string) => void>(() => undefined)
  Object.defineProperty(URL, 'createObjectURL', {
    value: createObjectURL,
    configurable: true,
    writable: true,
  })
  Object.defineProperty(URL, 'revokeObjectURL', {
    value: revokeObjectURL,
    configurable: true,
    writable: true,
  })
  return { createObjectURL, revokeObjectURL }
}

function restoreObjectUrls(): void {
  for (const [key, descriptor] of [
    ['createObjectURL', ORIGINAL_CREATE_OBJECT_URL],
    ['revokeObjectURL', ORIGINAL_REVOKE_OBJECT_URL],
  ] as const) {
    if (descriptor === undefined) {
      Reflect.deleteProperty(URL, key)
    } else {
      Object.defineProperty(URL, key, descriptor)
    }
  }
}

function asAnchor(value: unknown): HTMLAnchorElement {
  if (!(value instanceof HTMLAnchorElement)) {
    throw new Error('Expected the click to be on an anchor.')
  }
  return value
}

function asInput(value: unknown): HTMLInputElement {
  if (!(value instanceof HTMLInputElement)) {
    throw new Error('Expected the click to be on an input.')
  }
  return value
}

// A FileList stand-in, which jsdom cannot construct.
function fileList(files: readonly File[]): Record<number, File> & {
  readonly length: number
  item(index: number): File | null
} {
  return Object.assign(Object.fromEntries(files.map((file, index) => [index, file])), {
    length: files.length,
    item: (index: number) => files[index] ?? null,
  })
}

afterEach(() => {
  Reflect.deleteProperty(window, 'showOpenFilePicker')
  Reflect.deleteProperty(window, 'showSaveFilePicker')
  restoreObjectUrls()
  vi.restoreAllMocks()
  vi.useRealTimers()
  document.body.replaceChildren()
})

describe('FILE_TYPES', () => {
  it('describes each file kind with its media type and extensions', () => {
    expect(FILE_TYPES).toEqual({
      ptsm: {
        description: 'Partsmith model',
        mediaType: 'application/json',
        extensions: ['.ptsm'],
      },
      '3mf': { description: '3MF model', mediaType: 'model/3mf', extensions: ['.3mf'] },
      stl: { description: 'STL mesh', mediaType: 'model/stl', extensions: ['.stl'] },
      obj: { description: 'OBJ mesh', mediaType: 'model/obj', extensions: ['.obj'] },
      step: { description: 'STEP model', mediaType: 'model/step', extensions: ['.step', '.stp'] },
    })
  })
})

describe('web file service with the File System Access API', () => {
  it('reports fileSystemAccess when both pickers are functions', () => {
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    expect(createWebFileService(window).fileSystemAccess).toBe(true)
  })

  it('does not report fileSystemAccess with only the open picker', () => {
    Object.defineProperty(window, 'showOpenFilePicker', {
      value: unusedPicker(),
      configurable: true,
      writable: true,
    })
    expect(createWebFileService(window).fileSystemAccess).toBe(false)
  })

  it('U-74 pickToOpen returns the name, bytes and handle', async () => {
    const fake = fakeHandle()
    const open = vi.fn<() => Promise<FileSystemFileHandle[]>>(async () => [fake.handle])
    provideFileSystemAccess(open, unusedPicker())
    const opened = await createWebFileService(window).pickToOpen()
    expect(opened?.name).toBe('bracket.ptsm')
    expect(opened?.bytes).toEqual(BYTES)
    expect(opened?.handle).toBe(fake.handle)
  })

  it('pickToOpen asks for one .ptsm file and keeps the all-files option', async () => {
    const open = vi.fn<() => Promise<FileSystemFileHandle[]>>(async () => [fakeHandle().handle])
    provideFileSystemAccess(open, unusedPicker())
    await createWebFileService(window).pickToOpen()
    expect(open).toHaveBeenCalledWith({
      types: [PTSM_PICKER_TYPE],
      excludeAcceptAllOption: false,
      multiple: false,
    })
  })

  it('U-74 pickToOpen returns null on an AbortError', async () => {
    const open = vi.fn<() => Promise<never>>(async () => {
      throw abortError()
    })
    provideFileSystemAccess(open, unusedPicker())
    expect(await createWebFileService(window).pickToOpen()).toBeNull()
  })

  it('U-74 chooseSaveTarget returns a handle target named after the chosen handle', async () => {
    const fake = fakeHandle({ name: 'bracket-v2.ptsm' })
    const save = vi.fn<() => Promise<FileSystemFileHandle>>(async () => fake.handle)
    provideFileSystemAccess(unusedPicker(), save)
    const target = await createWebFileService(window).chooseSaveTarget('bracket.ptsm', 'ptsm')
    expect(target).toEqual({ kind: 'handle', name: 'bracket-v2.ptsm', handle: fake.handle })
    expect(save).toHaveBeenCalledWith({ suggestedName: 'bracket.ptsm', types: [PTSM_PICKER_TYPE] })
  })

  it('chooseSaveTarget offers the file type of the kind', async () => {
    const save = vi.fn<() => Promise<FileSystemFileHandle>>(
      async () => fakeHandle({ name: 'cube.step' }).handle,
    )
    provideFileSystemAccess(unusedPicker(), save)
    await createWebFileService(window).chooseSaveTarget('cube.step', 'step')
    expect(save).toHaveBeenCalledWith({
      suggestedName: 'cube.step',
      types: [{ description: 'STEP model', accept: { 'model/step': ['.step', '.stp'] } }],
    })
  })

  it('chooseSaveTarget returns null on an AbortError', async () => {
    const save = vi.fn<() => Promise<never>>(async () => {
      throw abortError()
    })
    provideFileSystemAccess(unusedPicker(), save)
    expect(await createWebFileService(window).chooseSaveTarget('bracket.ptsm', 'ptsm')).toBeNull()
  })

  it('U-74 write calls requestPermission when queryPermission is prompt, then writes and closes', async () => {
    const fake = fakeHandle({ permission: 'prompt', requestResult: 'granted' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    await createWebFileService(window).write(
      { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
      'ptsm',
      BYTES,
    )
    expect(fake.queryPermission).toHaveBeenCalledWith({ mode: 'readwrite' })
    expect(fake.requestPermission).toHaveBeenCalledWith({ mode: 'readwrite' })
    expect(fake.write).toHaveBeenCalledWith(BYTES)
    expect(fake.close).toHaveBeenCalledTimes(1)
    expect(fake.write.mock.invocationCallOrder[0]).toBeLessThan(
      fake.close.mock.invocationCallOrder[0] ?? 0,
    )
  })

  it('write does not ask again when the permission is granted', async () => {
    const fake = fakeHandle({ permission: 'granted' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    await createWebFileService(window).write(
      { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
      'ptsm',
      BYTES,
    )
    expect(fake.requestPermission).not.toHaveBeenCalled()
    expect(fake.write).toHaveBeenCalledWith(BYTES)
  })

  it('U-74 write throws FileServiceError permission when the request is denied', async () => {
    const fake = fakeHandle({ permission: 'prompt', requestResult: 'denied' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(
      createWebFileService(window).write(
        { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
        'ptsm',
        BYTES,
      ),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'permission' })
    expect(fake.createWritable).not.toHaveBeenCalled()
  })

  it('write throws FileServiceError permission when the permission is denied', async () => {
    const fake = fakeHandle({ permission: 'denied' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(
      createWebFileService(window).write(
        { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
        'ptsm',
        BYTES,
      ),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'permission' })
    expect(fake.createWritable).not.toHaveBeenCalled()
  })

  it('U-74 a NotFoundError while writing becomes FileServiceError not_found', async () => {
    const fake = fakeHandle({ createWritableError: notFoundError() })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(
      createWebFileService(window).write(
        { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
        'ptsm',
        BYTES,
      ),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'not_found' })
  })

  it('any other write failure becomes FileServiceError io with the original message', async () => {
    const fake = fakeHandle({ writeError: new Error('The disk is full.') })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(
      createWebFileService(window).write(
        { kind: 'handle', name: 'bracket.ptsm', handle: fake.handle },
        'ptsm',
        BYTES,
      ),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'io', message: 'The disk is full.' })
  })

  it('readHandle reads a handle with read permission', async () => {
    const fake = fakeHandle()
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const opened = await createWebFileService(window).readHandle(fake.handle)
    expect(fake.queryPermission).toHaveBeenCalledWith({ mode: 'read' })
    expect(fake.requestPermission).not.toHaveBeenCalled()
    expect(opened.name).toBe('bracket.ptsm')
    expect(opened.bytes).toEqual(BYTES)
    expect(opened.handle).toBe(fake.handle)
  })

  it('readHandle requests permission when it is prompt', async () => {
    const fake = fakeHandle({ permission: 'prompt', requestResult: 'granted' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const opened = await createWebFileService(window).readHandle(fake.handle)
    expect(fake.requestPermission).toHaveBeenCalledWith({ mode: 'read' })
    expect(opened.bytes).toEqual(BYTES)
  })

  it('readHandle throws FileServiceError permission when not granted', async () => {
    const fake = fakeHandle({ permission: 'prompt', requestResult: 'denied' })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(createWebFileService(window).readHandle(fake.handle))
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'permission' })
    expect(fake.getFile).not.toHaveBeenCalled()
  })

  it('readHandle turns a NotFoundError into FileServiceError not_found', async () => {
    const fake = fakeHandle({ getFileError: notFoundError() })
    provideFileSystemAccess(unusedPicker(), unusedPicker())
    const error = await rejectionOf(createWebFileService(window).readHandle(fake.handle))
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'not_found' })
  })
})

describe('desktop handles in the web file service', () => {
  it('desktop test 42: readHandle of a desktop handle is a permission error naming the file', async () => {
    const handle: DesktopFileHandle = { kind: 'desktop', name: 'a.ptsm', grant: 'g1' }

    const error = await rejectionOf(createWebFileService(window).readHandle(handle))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'permission',
      message: 'Partsmith has no access to a.ptsm.',
    })
  })
})

describe('isFileHandle', () => {
  it('desktop test 45: accepts a desktop handle', () => {
    expect(isFileHandle({ kind: 'desktop', name: 'a', grant: 'g' })).toBe(true)
  })

  it('desktop test 45: accepts a web handle', () => {
    expect(isFileHandle(fakeHandle({ name: 'a.ptsm' }).handle)).toBe(true)
  })

  it('desktop test 45: rejects a desktop handle without a grant', () => {
    expect(isFileHandle({ kind: 'desktop', name: 'a' })).toBe(false)
  })

  it('desktop test 45: rejects a desktop handle without a name', () => {
    expect(isFileHandle({ kind: 'desktop', grant: 'g' })).toBe(false)
  })
})

describe('web file service without the File System Access API', () => {
  it('does not report fileSystemAccess', () => {
    expect(createWebFileService(window).fileSystemAccess).toBe(false)
  })

  it('U-75 chooseSaveTarget returns a download target with the suggested name at once', async () => {
    const target = await createWebFileService(window).chooseSaveTarget('bracket.ptsm', 'ptsm')
    expect(target).toEqual({ kind: 'download', name: 'bracket.ptsm' })
  })

  it('U-75 write clicks an anchor whose download is the name and revokes its object URL after the click', async () => {
    const { createObjectURL, revokeObjectURL } = stubObjectUrls()
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined)
    vi.useFakeTimers()
    const writing = createWebFileService(window).write(
      { kind: 'download', name: 'bracket.ptsm' },
      'ptsm',
      BYTES,
    )
    await vi.runAllTimersAsync()
    await writing
    vi.useRealTimers()
    expect(click).toHaveBeenCalledTimes(1)
    const anchor = asAnchor(click.mock.contexts[0])
    expect(anchor.download).toBe('bracket.ptsm')
    expect(anchor.getAttribute('href')).toBe('blob:http://localhost/3f2c')
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:http://localhost/3f2c')
    expect(click.mock.invocationCallOrder[0]).toBeLessThan(
      revokeObjectURL.mock.invocationCallOrder[0] ?? 0,
    )
    expect(createObjectURL).toHaveBeenCalledTimes(1)
  })

  it('write downloads a Blob of the bytes with the media type of the kind', async () => {
    const { createObjectURL } = stubObjectUrls()
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined)
    vi.useFakeTimers()
    const writing = createWebFileService(window).write(
      { kind: 'download', name: 'cube.3mf' },
      '3mf',
      BYTES,
    )
    await vi.runAllTimersAsync()
    await writing
    vi.useRealTimers()
    const blob = createObjectURL.mock.calls[0]?.[0]
    expect(blob).toBeInstanceOf(Blob)
    expect(blob?.type).toBe('model/3mf')
    expect(new Uint8Array((await blob?.arrayBuffer()) ?? new ArrayBuffer(0))).toEqual(BYTES)
  })

  it('U-75 pickToOpen clicks a .ptsm file input and resolves from its change event', async () => {
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => undefined)
    const opening = createWebFileService(window).pickToOpen()
    await vi.waitFor(() => {
      expect(click).toHaveBeenCalledTimes(1)
    })
    const input = asInput(click.mock.contexts[0])
    expect(input.type).toBe('file')
    expect(input.accept).toBe('.ptsm')
    const file = new File([CONTENT], 'bracket.ptsm', { type: 'application/json' })
    fireEvent.change(input, { target: { files: fileList([file]) } })
    const opened = await opening
    expect(opened?.name).toBe('bracket.ptsm')
    expect(opened?.bytes).toEqual(BYTES)
    expect(opened?.handle).toBeNull()
  })

  it('pickToOpen resolves null when the input reports no file', async () => {
    const click = vi.spyOn(HTMLInputElement.prototype, 'click').mockImplementation(() => undefined)
    const opening = createWebFileService(window).pickToOpen()
    await vi.waitFor(() => {
      expect(click).toHaveBeenCalledTimes(1)
    })
    fireEvent.change(asInput(click.mock.contexts[0]), { target: { files: fileList([]) } })
    expect(await opening).toBeNull()
  })
})
