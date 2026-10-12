import { describe, expect, it } from 'vitest'
import { createFakeFileHandle } from '../test/fakes/files'
import { createFakeTauri, rejects, resolves, type FakeScript } from '../test/fakes/tauri'
import { createDesktopFileService, GRANT_HEADER, tauriGlobalOf } from './desktop-file-service'
import { FileServiceError, type DesktopFileHandle } from './file-service'

// The bytes of "hello".
const HELLO = new Uint8Array([0x68, 0x65, 0x6c, 0x6c, 0x6f])

const PART: DesktopFileHandle = { kind: 'desktop', name: 'part.ptsm', grant: 'g1' }
const BRACKET: DesktopFileHandle = { kind: 'desktop', name: 'bracket.ptsm', grant: 'g2' }

/** A fresh ArrayBuffer holding `bytes`, as the host's raw response would be. */
function arrayBufferOf(bytes: Uint8Array): ArrayBuffer {
  return bytes.slice().buffer
}

function serviceAnswering(script: FakeScript) {
  const fake = createFakeTauri(script)
  return { service: createDesktopFileService(fake.tauri), invocations: fake.invocations }
}

async function rejectionOf(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise
  } catch (error) {
    return error
  }
  throw new Error('The promise resolved.')
}

function asArrayBuffer(value: unknown): ArrayBuffer {
  if (!(value instanceof ArrayBuffer)) {
    throw new Error('Expected the payload to be an ArrayBuffer.')
  }
  return value
}

describe('createDesktopFileService', () => {
  it('desktop test 36: pickToOpen reads the chosen file through its grant', async () => {
    const { service, invocations } = serviceAnswering({
      pick_open_file: [resolves({ name: 'part.ptsm', grant: 'g1' })],
      read_file: [resolves(arrayBufferOf(HELLO))],
    })

    const opened = await service.pickToOpen()

    expect(opened).toEqual({ name: 'part.ptsm', bytes: HELLO, handle: PART })
    expect(invocations.map((invocation) => invocation.command)).toEqual([
      'pick_open_file',
      'read_file',
    ])
    expect(invocations[1]?.payload).toEqual({ grant: 'g1' })
  })

  it('desktop test 37: pickToOpen resolves null when the user cancels, without reading', async () => {
    const { service, invocations } = serviceAnswering({ pick_open_file: [resolves(null)] })

    expect(await service.pickToOpen()).toBeNull()
    expect(invocations).toHaveLength(1)
  })

  it('desktop test 38: chooseSaveTarget asks the host with the suggested name and kind', async () => {
    const { service, invocations } = serviceAnswering({
      pick_save_file: [resolves({ name: 'bracket.ptsm', grant: 'g2' })],
    })

    await service.chooseSaveTarget('bracket.ptsm', 'ptsm')

    expect(invocations).toHaveLength(1)
    expect(invocations[0]?.command).toBe('pick_save_file')
    expect(invocations[0]?.payload).toEqual({ suggestedName: 'bracket.ptsm', kind: 'ptsm' })
  })

  it('desktop test 38: a chosen save file becomes a handle target with its desktop handle', async () => {
    const { service } = serviceAnswering({
      pick_save_file: [resolves({ name: 'bracket.ptsm', grant: 'g2' })],
    })

    const target = await service.chooseSaveTarget('bracket.ptsm', 'ptsm')

    expect(target).toEqual({ kind: 'handle', name: 'bracket.ptsm', handle: BRACKET })
  })

  it('desktop test 38: chooseSaveTarget resolves null when the user cancels', async () => {
    const { service } = serviceAnswering({ pick_save_file: [resolves(null)] })

    expect(await service.chooseSaveTarget('bracket.ptsm', 'ptsm')).toBeNull()
  })

  it('chooseSaveTarget names the target after the file the host granted', async () => {
    const { service } = serviceAnswering({
      pick_save_file: [resolves({ name: 'bracket.ptsm', grant: 'g2' })],
    })

    const target = await service.chooseSaveTarget('bracket', 'ptsm')

    expect(target).toEqual({ kind: 'handle', name: 'bracket.ptsm', handle: BRACKET })
  })

  it('desktop test 39: write sends the bytes as an ArrayBuffer with the grant header', async () => {
    const { service, invocations } = serviceAnswering({ write_file: [resolves(null)] })

    await service.write(
      { kind: 'handle', name: 'bracket.ptsm', handle: BRACKET },
      'ptsm',
      new Uint8Array([1, 2, 3]),
    )

    expect(invocations).toHaveLength(1)
    expect(invocations[0]?.command).toBe('write_file')
    expect(new Uint8Array(asArrayBuffer(invocations[0]?.payload))).toEqual(
      new Uint8Array([1, 2, 3]),
    )
    expect(invocations[0]?.options).toEqual({ headers: { 'Partsmith-Grant': 'g2' } })
  })

  it('desktop test 39: write of a view into a larger buffer sends exactly the viewed bytes', async () => {
    const { service, invocations } = serviceAnswering({ write_file: [resolves(null)] })
    const buffer = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7]).buffer

    await service.write(
      { kind: 'handle', name: 'bracket.ptsm', handle: BRACKET },
      'ptsm',
      new Uint8Array(buffer, 2, 3),
    )

    const payload = asArrayBuffer(invocations[0]?.payload)
    expect(payload.byteLength).toBe(3)
    expect(new Uint8Array(payload)).toEqual(new Uint8Array([2, 3, 4]))
  })

  it('write sends the buffer of a view that covers all of it without copying', async () => {
    const { service, invocations } = serviceAnswering({ write_file: [resolves(null)] })
    const bytes = new Uint8Array([1, 2, 3])

    await service.write({ kind: 'handle', name: 'bracket.ptsm', handle: BRACKET }, 'ptsm', bytes)

    expect(invocations[0]?.payload).toBe(bytes.buffer)
  })

  it('GRANT_HEADER is Partsmith-Grant', () => {
    expect(GRANT_HEADER).toBe('Partsmith-Grant')
  })

  it('readHandle of a desktop handle reads it through read_file', async () => {
    const { service, invocations } = serviceAnswering({
      read_file: [resolves(arrayBufferOf(HELLO))],
    })

    const opened = await service.readHandle(PART)

    expect(opened).toEqual({ name: 'part.ptsm', bytes: HELLO, handle: PART })
    expect(invocations.map(({ command, payload }) => [command, payload])).toEqual([
      ['read_file', { grant: 'g1' }],
    ])
  })

  it('desktop test 40: a not_found rejection becomes FileServiceError not_found with its message', async () => {
    const { service } = serviceAnswering({
      read_file: [rejects({ reason: 'not_found', message: 'part.ptsm was not found.' })],
    })

    const error = await rejectionOf(service.readHandle(PART))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'not_found', message: 'part.ptsm was not found.' })
  })

  it('desktop test 40: a permission rejection becomes FileServiceError permission', async () => {
    const { service } = serviceAnswering({
      read_file: [rejects({ reason: 'permission', message: 'x' })],
    })

    const error = await rejectionOf(service.readHandle(PART))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'permission', message: 'x' })
  })

  it('desktop test 40: a string rejection from Tauri becomes FileServiceError io with that text', async () => {
    const { service } = serviceAnswering({
      read_file: [rejects('Command read_file not allowed by ACL')],
    })

    const error = await rejectionOf(service.readHandle(PART))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'io', message: 'Command read_file not allowed by ACL' })
  })

  it('a rejection becomes the cause of the FileServiceError', async () => {
    const rejection = { reason: 'io', message: 'No space left on device (os error 28)' }
    const { service } = serviceAnswering({ write_file: [rejects(rejection)] })

    const error = await rejectionOf(
      service.write({ kind: 'handle', name: 'bracket.ptsm', handle: BRACKET }, 'ptsm', HELLO),
    )

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'No space left on device (os error 28)',
      cause: rejection,
    })
  })

  it('an Error rejection becomes FileServiceError io with its message', async () => {
    const { service } = serviceAnswering({
      pick_open_file: [rejects(new Error('The dialog failed.'))],
    })

    const error = await rejectionOf(service.pickToOpen())

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'io', message: 'The dialog failed.' })
  })

  it('any other rejection becomes FileServiceError io with its text', async () => {
    const { service } = serviceAnswering({ pick_open_file: [rejects(42)] })

    const error = await rejectionOf(service.pickToOpen())

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({ reason: 'io', message: '42' })
  })

  it('desktop test 41: a pick reply that is not a chosen file is an unexpected reply', async () => {
    const { service } = serviceAnswering({ pick_open_file: [resolves(42)] })

    const error = await rejectionOf(service.pickToOpen())

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'The desktop app sent an unexpected reply to pick_open_file.',
    })
  })

  it('desktop test 41: a read reply that is not an ArrayBuffer is an unexpected reply', async () => {
    const { service } = serviceAnswering({ read_file: [resolves('hello')] })

    const error = await rejectionOf(service.readHandle(PART))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'The desktop app sent an unexpected reply to read_file.',
    })
  })

  it.each([
    ['an empty name', { name: '', grant: 'g1' }],
    ['an empty grant', { name: 'part.ptsm', grant: '' }],
    ['no grant', { name: 'part.ptsm' }],
  ])('a pick reply with %s is an unexpected reply', async (_, reply) => {
    const { service } = serviceAnswering({ pick_save_file: [resolves(reply)] })

    const error = await rejectionOf(service.chooseSaveTarget('part.ptsm', 'ptsm'))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'The desktop app sent an unexpected reply to pick_save_file.',
    })
  })

  it('desktop test 42: write to a download target is an io error without asking the host', async () => {
    const { service, invocations } = serviceAnswering({})

    const error = await rejectionOf(
      service.write({ kind: 'download', name: 'x.stl' }, 'stl', HELLO),
    )

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'io',
      message: 'The desktop app saves to files, not downloads.',
    })
    expect(invocations).toEqual([])
  })

  it('desktop test 42: readHandle of a web handle is a permission error naming the file', async () => {
    const { service, invocations } = serviceAnswering({})

    const error = await rejectionOf(service.readHandle(createFakeFileHandle('a.ptsm')))

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'permission',
      message: 'Partsmith has no access to a.ptsm.',
    })
    expect(invocations).toEqual([])
  })

  it('write to a web handle target is a permission error naming the file', async () => {
    const { service, invocations } = serviceAnswering({})
    const handle = createFakeFileHandle('a.ptsm')

    const error = await rejectionOf(
      service.write({ kind: 'handle', name: 'a.ptsm', handle }, 'ptsm', HELLO),
    )

    expect(error).toBeInstanceOf(FileServiceError)
    expect(error).toMatchObject({
      reason: 'permission',
      message: 'Partsmith has no access to a.ptsm.',
    })
    expect(invocations).toEqual([])
  })

  it('desktop test 43: the desktop service always has file system access', () => {
    expect(serviceAnswering({}).service.fileSystemAccess).toBe(true)
  })
})

describe('tauriGlobalOf', () => {
  it('desktop test 44: an environment without __TAURI__ has no Tauri global', () => {
    expect(tauriGlobalOf({})).toBeNull()
  })

  it('desktop test 44: an environment whose __TAURI__.core has no invoke has no Tauri global', () => {
    expect(tauriGlobalOf({ __TAURI__: { core: {} } })).toBeNull()
  })

  it('returns __TAURI__ when its core.invoke is a function', () => {
    const { tauri } = createFakeTauri()

    expect(tauriGlobalOf({ __TAURI__: tauri })).toBe(tauri)
  })
})
