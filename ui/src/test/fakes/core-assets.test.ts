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
  return readResponse(await transport.send(requestFrame(1, command, buffers), 1))
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

/** The `document_changed` event of a response; fails the test when there is none. */
function documentChanged(events: readonly ProtocolEvent[]): ProtocolEvent & { type: 'document_changed' } {
  const changed = events.find((event) => event.type === 'document_changed')
  if (changed?.type !== 'document_changed') {
    throw new Error('The response has no document_changed.')
  }
  return changed
}

/** Opens `block.ptsm` without assets and returns the transport. */
async function blockOpenedBare(
  options: { readonly mismatchedAssets?: readonly string[] } = {},
): Promise<FakeTransport> {
  const transport = await started(options)
  await send(transport, { type: 'open_document', text: documentText(BLOCK_DOCUMENT), assets: [] })
  return transport
}

describe('the fake core adds assets as the engine does (since M2, ADR 0064)', () => {
  it('reports the asset an import reads and it lacks in document_changed', async () => {
    const transport = await started()

    const opened = await send(transport, {
      type: 'open_document',
      text: documentText(BLOCK_DOCUMENT),
      assets: [],
    })

    expect(documentChanged(opened.events).missing_assets).toEqual([{ name: A, features: ['f-imp1'] }])
  })

  it('stores a named file, then emits document_changed and recomputed', async () => {
    const transport = await blockOpenedBare()

    const added = await send(
      transport,
      { type: 'add_assets', doc: 1, assets: [{ kind: 'step', data: 0, name: A }] },
      [ASSET_A.bytes],
    )

    expect(added.response.result).toMatchObject({
      kind: 'ok',
      type: 'add_assets',
      assets: [{ name: A, stored: true }],
    })
    expect(added.events.map((event) => event.type).slice(0, 2)).toEqual([
      'document_changed',
      'recomputed',
    ])
    expect(documentChanged(added.events).missing_assets).toEqual([])
    expect(importStatus(added.events)).toMatchObject({ status: 'ok', messages: [] })
    expect(transport.core.document(1).assets).toEqual([A])
  })

  it('stores no named file its mismatchedAssets option lists, and emits nothing', async () => {
    const transport = await blockOpenedBare({ mismatchedAssets: [A] })

    const added = await send(
      transport,
      { type: 'add_assets', doc: 1, assets: [{ kind: 'step', data: 0, name: A }] },
      [ASSET_A.bytes],
    )

    expect(added.response.result).toMatchObject({ assets: [{ name: A, stored: false }] })
    expect(added.events).toEqual([])
    expect(transport.core.document(1).assets).toEqual([])
  })

  it('names a file without a name by 64 c digits and its kind, and emits nothing when no record reads it', async () => {
    const transport = await blockOpenedBare()

    const added = await send(
      transport,
      {
        type: 'add_assets',
        doc: 1,
        assets: [
          { kind: 'step', data: 0 },
          { kind: 'ttf', data: 1 },
        ],
      },
      [Uint8Array.of(7, 8), Uint8Array.of(9)],
    )

    expect(added.response.result).toMatchObject({
      assets: [
        { name: `${'c'.repeat(64)}.step`, stored: true },
        { name: `${'c'.repeat(64)}.ttf`, stored: true },
      ],
    })
    expect(added.events).toEqual([])
  })
})
