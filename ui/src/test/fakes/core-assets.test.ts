import { describe, expect, it } from 'vitest'
import type { Command, Event as ProtocolEvent } from '../../generated/protocol'
import {
  ASSET_A,
  ASSET_B,
  ASSET_MISSING_MESSAGE,
  assetMismatchMessage,
  BLOCK_DOCUMENT,
} from './assets'
import { documentText } from './documents'
import { readResponse, requestFrame } from './protocol'
import { createFakeTransport, type FakeTransport } from './transport'

// The fake core's assets (ui.md 6.4, the preamble of U-216 to U-232): it keeps the assets
// open_document supplies, fails an import whose asset it does not hold with asset_missing,
// refuses a supplied name its mismatchedAssets option lists, and saves the assets the text names.

const A = ASSET_A.name
const B = ASSET_B.name

async function started(
  options: { readonly mismatchedAssets?: readonly string[] } = {},
): Promise<FakeTransport> {
  const transport = createFakeTransport(options)
  await transport.start()
  return transport
}

/** Sends one command with buffers and decodes the answer. */
async function send(
  transport: FakeTransport,
  command: Command,
  buffers: readonly Uint8Array[] = [],
): Promise<ReturnType<typeof readResponse>> {
  return readResponse(await transport.send(requestFrame(1, command, buffers)))
}

/** The status of `f-imp1` in the recompute of a response. */
function importStatus(events: readonly ProtocolEvent[]): unknown {
  const recomputed = events.find((event) => event.type === 'recomputed')
  return recomputed?.type === 'recomputed'
    ? recomputed.features.find((feature) => feature.id === 'f-imp1')
    : undefined
}

describe('the fake core keeps the assets of a document', () => {
  it('opens an import ok with its asset and saves that asset as a u8 buffer', async () => {
    const transport = await started()
    const opened = await send(
      transport,
      {
        type: 'open_document',
        text: documentText(BLOCK_DOCUMENT),
        assets: [
          { name: A, data: 0 },
          { name: B, data: 1 },
        ],
      },
      [ASSET_A.bytes, ASSET_B.bytes],
    )
    expect(importStatus(opened.events)).toMatchObject({ status: 'ok', messages: [] })
    expect(transport.core.document(1).assets).toEqual([A, B])

    const saved = await send(transport, { type: 'save_document', doc: 1 })
    expect(saved.response.result).toMatchObject({
      kind: 'ok',
      type: 'save_document',
      assets: [{ name: A, data: 0 }],
    })
    expect(saved.buffers.map((buffer) => Array.from(buffer))).toEqual([[1, 2, 3]])
  })

  it('fails an import whose asset it does not hold with asset_missing, naming the asset in detail', async () => {
    const transport = await started()
    const opened = await send(transport, {
      type: 'open_document',
      text: documentText(BLOCK_DOCUMENT),
      assets: [],
    })
    expect(importStatus(opened.events)).toMatchObject({
      status: 'error',
      messages: [
        { severity: 'error', code: 'asset_missing', message: ASSET_MISSING_MESSAGE, detail: A },
      ],
    })
    const saved = await send(transport, { type: 'save_document', doc: 1 })
    expect(saved.response.result).toMatchObject({ kind: 'ok', assets: [] })
  })

  it('refuses a referenced asset its mismatchedAssets option lists, and ignores an unreferenced one', async () => {
    const transport = await started({ mismatchedAssets: [A, B] })
    const refused = await send(
      transport,
      { type: 'open_document', text: documentText(BLOCK_DOCUMENT), assets: [{ name: A, data: 0 }] },
      [ASSET_A.bytes],
    )
    expect(refused.response.result).toMatchObject({
      kind: 'error',
      code: 'asset_mismatch',
      message: assetMismatchMessage(A),
    })
    expect(transport.core.handles).toEqual([])

    const unreferenced = await send(
      transport,
      { type: 'open_document', text: documentText(BLOCK_DOCUMENT), assets: [{ name: B, data: 0 }] },
      [ASSET_B.bytes],
    )
    expect(unreferenced.response.result).toMatchObject({ kind: 'ok', type: 'open_document' })
  })

  it('records the buffers of each request', async () => {
    const transport = await started()
    await send(
      transport,
      { type: 'open_document', text: documentText(BLOCK_DOCUMENT), assets: [{ name: A, data: 0 }] },
      [ASSET_A.bytes],
    )
    const [command] = transport.commandsOfType('open_document')
    if (command === undefined) {
      throw new Error('No open_document was recorded.')
    }
    expect(transport.buffersOf(command).map((buffer) => Array.from(buffer))).toEqual([[1, 2, 3]])
  })
})
