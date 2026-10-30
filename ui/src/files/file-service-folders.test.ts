import { afterEach, describe, expect, it, vi, type Mock } from 'vitest'
import { ASSET_A, ASSET_B, ASSET_C_NAME } from '../test/fakes/assets'
import { rejectionOf } from '../test/fakes/deferred'
import {
  domError,
  fakeWebFile,
  fakeWebFolder,
  fileIn,
  folderIn,
  type FakeWebFile,
  type FakeWebFolder,
} from '../test/fakes/web-folders'
import {
  createWebFileService,
  FileServiceError,
  folderHolds,
  type DesktopFileHandle,
} from './file-service'

// The web file service's folder of a document (ui.md 2.15, 3.19, ADR 0037 decision 7): reading
// and writing assets through the folder the user chose, the entry check of file-format.md 3.11 as
// far as the API sees it, the folder dialog, and the order of permission requests. U-230 to U-232.

const A = ASSET_A.name
const B = ASSET_B.name

const DESKTOP: DesktopFileHandle = { kind: 'desktop', name: 'block.ptsm', grant: 'g1' }

afterEach(() => {
  Reflect.deleteProperty(window, 'showOpenFilePicker')
  Reflect.deleteProperty(window, 'showSaveFilePicker')
  Reflect.deleteProperty(window, 'showDirectoryPicker')
  vi.restoreAllMocks()
})

function service(): ReturnType<typeof createWebFileService> {
  return createWebFileService(window)
}

/** Gives the window all three pickers; returns the directory picker. */
function provideDirectoryPicker(
  answer: () => Promise<FileSystemDirectoryHandle>,
): Mock<(options: unknown) => Promise<FileSystemDirectoryHandle>> {
  const unused = vi.fn<() => Promise<never>>(async () => {
    throw new Error('This picker is not expected to be called.')
  })
  const directory = vi.fn<(options: unknown) => Promise<FileSystemDirectoryHandle>>(answer)
  for (const [key, value] of [
    ['showOpenFilePicker', unused],
    ['showSaveFilePicker', unused],
    ['showDirectoryPicker', directory],
  ] as const) {
    Object.defineProperty(window, key, { value, configurable: true, writable: true })
  }
  return directory
}

/** The folder `F` holding `block.ptsm` and, when given, an `assets` folder with these entries. */
function documentFolder(assets?: FakeWebFolder | FakeWebFile): {
  readonly folder: FakeWebFolder
  readonly document: FakeWebFile
} {
  const document = fakeWebFile('block.ptsm', Uint8Array.of(123, 125))
  const folder = fakeWebFolder('work', assets === undefined ? [document] : [document, assets])
  return { folder, document }
}

describe('FileServiceError (2.15)', () => {
  it('has entry null unless given, and keeps the cause', () => {
    const cause = new Error('denied')
    expect(new FileServiceError('io', 'x').entry).toBeNull()
    const error = new FileServiceError('permission', 'y', { entry: 'assets', cause })
    expect(error.entry).toBe('assets')
    expect(error.reason).toBe('permission')
    expect(error.message).toBe('y')
    expect(error.cause).toBe(cause)
  })
})

describe('web readAssets (2.15)', () => {
  it('U-230 a place without a folder reads nothing and calls nothing', async () => {
    const { document } = documentFolder()
    await expect(
      service().readAssets({ handle: document.handle, folder: null }, [A]),
    ).resolves.toEqual({ assets: [], failure: null })
    expect(document.getFile).not.toHaveBeenCalled()
    expect(document.queryPermission).not.toHaveBeenCalled()
  })

  it('U-230 reads the named files in order, leaves out a missing one and reports a folder under a name', async () => {
    const assets = fakeWebFolder('assets', [fakeWebFile(A, ASSET_A.bytes), fakeWebFolder(B)])
    const { folder, document } = documentFolder(assets)
    const reading = await service().readAssets({ handle: document.handle, folder: folder.handle }, [
      A,
      B,
      ASSET_C_NAME,
    ])
    expect(
      reading.assets.map((asset) => ({ name: asset.name, bytes: Array.from(asset.bytes) })),
    ).toEqual([{ name: A, bytes: [1, 2, 3] }])
    expect(reading.failure).toBeInstanceOf(FileServiceError)
    expect(reading.failure).toMatchObject({ reason: 'io', entry: `assets/${B}` })
    expect(assets.getFileHandle.mock.calls.map(([name]) => name)).toEqual([A, B, ASSET_C_NAME])
  })

  it('U-230 a folder without assets reads nothing and reports nothing', async () => {
    const { folder, document } = documentFolder()
    await expect(
      service().readAssets({ handle: document.handle, folder: folder.handle }, [A]),
    ).resolves.toEqual({ assets: [], failure: null })
  })

  it('U-230 a file named assets is an io failure for the folder', async () => {
    const { folder, document } = documentFolder(fakeWebFile('assets', Uint8Array.of(1)))
    const reading = await service().readAssets({ handle: document.handle, folder: folder.handle }, [
      A,
    ])
    expect(reading.assets).toEqual([])
    expect(reading.failure).toMatchObject({ reason: 'io', entry: 'assets' })
  })

  it("U-230 asks for the folder's readwrite permission when it is prompt", async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1))
    const folder = fakeWebFolder('work', [document], { permission: { query: 'prompt' } })
    await service().readAssets({ handle: document.handle, folder: folder.handle }, [A])
    expect(folder.queryPermission).toHaveBeenCalledWith({ mode: 'readwrite' })
    expect(folder.requestPermission).toHaveBeenCalledWith({ mode: 'readwrite' })
  })

  it('U-230 a refused folder permission is a permission failure for assets', async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1))
    const folder = fakeWebFolder('work', [document], {
      permission: { query: 'prompt', request: 'denied' },
    })
    const reading = await service().readAssets({ handle: document.handle, folder: folder.handle }, [
      A,
    ])
    expect(reading.assets).toEqual([])
    expect(reading.failure).toMatchObject({ reason: 'permission', entry: 'assets' })
    expect(folder.getDirectoryHandle).not.toHaveBeenCalled()
  })
})

describe('web writeAssets and its entry check (2.15, file-format.md 3.11)', () => {
  it('U-231 creates assets and writes a new asset through one writable', async () => {
    const { folder, document } = documentFolder()
    await service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A])
    const assets = folderIn(folder, 'assets')
    const written = fileIn(assets, A)
    expect(Array.from(written.bytes)).toEqual([1, 2, 3])
    expect(written.createWritable).toHaveBeenCalledTimes(1)
    expect(folder.getDirectoryHandle).toHaveBeenCalledWith('assets', { create: true })
  })

  it('U-231 keeps an asset whose file already holds its bytes, writing nothing', async () => {
    const { folder, document } = documentFolder()
    const place = { handle: document.handle, folder: folder.handle }
    await service().writeAssets(place, [ASSET_A])
    const written = fileIn(folderIn(folder, 'assets'), A)
    await service().writeAssets(place, [ASSET_A])
    expect(written.createWritable).toHaveBeenCalledTimes(1)
  })

  it('U-231 replaces a file of the same size with other bytes', async () => {
    const stale = fakeWebFile(A, Uint8Array.of(1, 2, 4))
    const { folder, document } = documentFolder(fakeWebFolder('assets', [stale]))
    await service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A])
    expect(Array.from(stale.bytes)).toEqual([1, 2, 3])
    expect(stale.createWritable).toHaveBeenCalledTimes(1)
  })

  it('U-231 replaces a file of another size without reading its bytes', async () => {
    const stale = fakeWebFile(A, Uint8Array.of(9))
    const { folder, document } = documentFolder(fakeWebFolder('assets', [stale]))
    await service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A])
    expect(Array.from(stale.bytes)).toEqual([1, 2, 3])
    for (const file of stale.files) {
      expect(file.arrayBuffer).not.toHaveBeenCalled()
    }
  })

  it("U-231 a folder under an asset's name fails for that entry, stays, and nothing after it is written", async () => {
    const blocking = fakeWebFolder(A)
    const assets = fakeWebFolder('assets', [blocking])
    const { folder, document } = documentFolder(assets)
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A, ASSET_B]),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'io', entry: `assets/${A}` })
    expect(assets.entries.get(A)).toBe(blocking)
    expect(assets.entries.has(B)).toBe(false)
  })

  it('U-231 a new entry whose write fails is removed, so no empty file is left', async () => {
    const assets = fakeWebFolder('assets')
    assets.failingNewFiles.add(A)
    const { folder, document } = documentFolder(assets)
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A]),
    )
    expect(error).toMatchObject({ entry: `assets/${A}` })
    expect(assets.removeEntry).toHaveBeenCalledWith(A)
    expect(assets.entries.has(A)).toBe(false)
  })

  it('U-231 an existing file whose write fails is aborted and keeps its bytes', async () => {
    const stale = fakeWebFile(A, Uint8Array.of(9))
    stale.writeError = domError('InvalidStateError', 'The disk is full.')
    const assets = fakeWebFolder('assets', [stale])
    const { folder, document } = documentFolder(assets)
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A]),
    )
    expect(error).toMatchObject({ reason: 'io', entry: `assets/${A}` })
    expect(stale.writables[0]?.abort).toHaveBeenCalled()
    expect(Array.from(stale.bytes)).toEqual([9])
    expect(assets.removeEntry).not.toHaveBeenCalled()
  })

  it('U-231 a place without a folder throws io for assets and calls nothing', async () => {
    const { document } = documentFolder()
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: null }, [ASSET_A]),
    )
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'Partsmith has no access to the folder of block.ptsm.',
      entry: 'assets',
    })
    expect(document.queryPermission).not.toHaveBeenCalled()
  })

  it('a file named assets is an io failure for the folder, and nothing is written', async () => {
    const blocking = fakeWebFile('assets', Uint8Array.of(1))
    const { folder, document } = documentFolder(blocking)
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A]),
    )
    expect(error).toMatchObject({ reason: 'io', entry: 'assets' })
    expect(folder.entries.get('assets')).toBe(blocking)
  })

  it('a refused folder permission is a permission failure for assets, and nothing is written', async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1))
    const folder = fakeWebFolder('work', [document], {
      permission: { query: 'prompt', request: 'denied' },
    })
    const error = await rejectionOf(
      service().writeAssets({ handle: document.handle, folder: folder.handle }, [ASSET_A]),
    )
    expect(error).toMatchObject({ reason: 'permission', entry: 'assets' })
    expect(folder.getDirectoryHandle).not.toHaveBeenCalled()
  })
})

describe('web folders and permissions (2.15)', () => {
  it("U-232 chooseFolder opens the directory dialog in the file's folder with readwrite", async () => {
    const { folder, document } = documentFolder()
    const picker = provideDirectoryPicker(async () => folder.handle)
    await expect(service().chooseFolder(document.handle)).resolves.toEqual({
      kind: 'chosen',
      folder: folder.handle,
    })
    expect(picker).toHaveBeenCalledWith({ mode: 'readwrite', startIn: document.handle })
  })

  it('U-232 a canceled dialog gives canceled', async () => {
    const { document } = documentFolder()
    provideDirectoryPicker(async () => {
      throw domError('AbortError')
    })
    await expect(service().chooseFolder(document.handle)).resolves.toEqual({ kind: 'canceled' })
  })

  it.each([
    ['is not inside it', async () => null],
    ['is in a subfolder', async () => ['sub', 'block.ptsm']],
    [
      'cannot be resolved',
      async () => {
        throw domError('NotFoundError')
      },
    ],
  ])(
    'U-232 a folder that holds no file of that name directly (it %s) gives other with its name',
    async (_, resolved) => {
      const { document } = documentFolder()
      const parts = fakeWebFolder('parts')
      parts.resolve.mockImplementation(resolved)
      provideDirectoryPicker(async () => parts.handle)
      await expect(service().chooseFolder(document.handle)).resolves.toEqual({
        kind: 'other',
        name: 'parts',
      })
    },
  )

  it('a dialog that fails otherwise throws io with its message', async () => {
    const { document } = documentFolder()
    provideDirectoryPicker(async () => {
      throw domError('SecurityError', 'Must be handling a user gesture to show a file picker.')
    })
    const error = await rejectionOf(service().chooseFolder(document.handle))
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'Must be handling a user gesture to show a file picker.',
    })
  })

  it('chooseFolder in a browser without showDirectoryPicker throws the permission error of readHandle', async () => {
    const { document } = documentFolder()
    const error = await rejectionOf(service().chooseFolder(document.handle))
    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'permission',
      message: 'Partsmith has no access to block.ptsm.',
    })
  })

  it('chooseFolder of a desktop handle throws the permission error of readHandle', async () => {
    provideDirectoryPicker(async () => fakeWebFolder('work').handle)
    const error = await rejectionOf(service().chooseFolder(DESKTOP))
    expect(error).toMatchObject({
      reason: 'permission',
      message: 'Partsmith has no access to block.ptsm.',
    })
  })

  it('U-232 folderHolds is true for a file directly in the folder and false for a desktop handle', async () => {
    const { folder, document } = documentFolder()
    await expect(folderHolds(folder.handle, document.handle)).resolves.toBe(true)
    await expect(folderHolds(folder.handle, DESKTOP)).resolves.toBe(false)
    await expect(folderHolds(fakeWebFolder('other').handle, document.handle)).resolves.toBe(false)
  })

  it('U-232 readHandle asks for readwrite when told, and read otherwise', async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1), {
      permission: { query: 'prompt' },
    })
    await service().readHandle(document.handle, 'readwrite')
    expect(document.queryPermission).toHaveBeenLastCalledWith({ mode: 'readwrite' })
    expect(document.requestPermission).toHaveBeenLastCalledWith({ mode: 'readwrite' })
    await service().readHandle(document.handle)
    expect(document.queryPermission).toHaveBeenLastCalledWith({ mode: 'read' })
    expect(document.requestPermission).toHaveBeenLastCalledWith({ mode: 'read' })
  })

  it("U-232 readHandle with the document's folder asks for the folder's readwrite permission first", async () => {
    const log: string[] = []
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1), {
      permission: { query: 'prompt' },
      log,
    })
    const folder = fakeWebFolder('work', [document], { permission: { query: 'prompt' }, log })
    await service().readHandle(document.handle, 'read', folder.handle)
    expect(log).toEqual([
      'work.queryPermission:readwrite',
      'work.requestPermission:readwrite',
      'block.ptsm.queryPermission:read',
      'block.ptsm.requestPermission:read',
    ])
  })

  it('U-232 readHandle reads the file when the folder is refused and the file is granted', async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(7, 8))
    const folder = fakeWebFolder('work', [document], {
      permission: { query: 'prompt', request: 'denied' },
    })
    const opened = await service().readHandle(document.handle, 'read', folder.handle)
    expect(Array.from(opened.bytes)).toEqual([7, 8])
    expect(opened.handle).toBe(document.handle)
  })

  it('U-232 reachesFolder is true only for a web handle with a folder', () => {
    const { folder, document } = documentFolder()
    expect(service().reachesFolder({ handle: document.handle, folder: folder.handle })).toBe(true)
    expect(service().reachesFolder({ handle: document.handle, folder: null })).toBe(false)
    expect(service().reachesFolder({ handle: DESKTOP, folder: null })).toBe(false)
  })

  it('the errors of readHandle and write have entry null', async () => {
    const document = fakeWebFile('block.ptsm', Uint8Array.of(1), {
      permission: { query: 'prompt', request: 'denied' },
    })
    const error = await rejectionOf(service().readHandle(document.handle))
    expect(error).toMatchObject({ reason: 'permission', entry: null })
  })
})
