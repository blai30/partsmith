import { afterEach, describe, expect, it } from 'vitest'
import { createFakeTauri, resolves } from '../test/fakes/tauri'
import { chooseFileService } from './choose-file-service'

afterEach(() => {
  Reflect.deleteProperty(window, '__TAURI__')
})

describe('chooseFileService', () => {
  it('desktop test 44: with __TAURI__.core.invoke it gives the desktop service over that invoke', async () => {
    const fake = createFakeTauri({ pick_open_file: [resolves(null)] })
    Object.defineProperty(window, '__TAURI__', {
      value: fake.tauri,
      configurable: true,
      writable: true,
    })

    const service = chooseFileService(window)
    await service.pickToOpen()

    expect(fake.invocations.map((invocation) => invocation.command)).toEqual(['pick_open_file'])
  })

  it('desktop test 44: without __TAURI__ it gives the web service', async () => {
    const service = chooseFileService(window)

    // jsdom has no File System Access API, so the web service saves by downloading.
    expect(service.fileSystemAccess).toBe(false)
    expect(await service.chooseSaveTarget('bracket.ptsm', 'ptsm')).toEqual({
      kind: 'download',
      name: 'bracket.ptsm',
    })
  })
})
