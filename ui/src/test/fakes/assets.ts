import type { AssetFile } from '../../files/assets'
import type { Document } from '../../generated/protocol'

// The asset files and the document of ui.md 6.4's tests of assets and files changed elsewhere
// (U-216 to U-232) and of desktop.md's UI tests 64 to 66.

/** The asset name of 64 copies of `digit` followed by `.step`. */
export function assetName(digit: string): string {
  return `${digit.repeat(64)}.step`
}

/** `A`: the asset of 64 `a` digits, holding the bytes 1, 2, 3. */
export const ASSET_A: AssetFile = { name: assetName('a'), bytes: Uint8Array.of(1, 2, 3) }

/** `B`: the asset of 64 `b` digits, holding the bytes 4, 5. */
export const ASSET_B: AssetFile = { name: assetName('b'), bytes: Uint8Array.of(4, 5) }

/** The name of `C`, an asset no folder holds. */
export const ASSET_C_NAME = assetName('c')

/** `block.ptsm`: body `b-1` "Body 1" and `f-imp1`, an import of `A` into it, op new. */
export const BLOCK_DOCUMENT: Document = {
  format: 'partsmith',
  format_version: 2,
  units: 'mm',
  parameters: [],
  bodies: [{ id: 'b-1', name: 'Body 1' }],
  features: [{ id: 'f-imp1', type: 'import', body: 'b-1', asset: ASSET_A.name, op: 'new' }],
}

/** The message the engine gives an import whose asset it does not hold (features.md 4.1). */
export const ASSET_MISSING_MESSAGE =
  "The STEP file of this import is not in this document's assets folder. Copy the assets folder that came with the document next to it, or import the file again."

/** The engine's refusal of a supplied asset whose bytes do not hash to its name (protocol.md 4). */
export function assetMismatchMessage(name: string): string {
  return `Asset ${name} does not match its content hash.`
}

/** The names and bytes of asset files as plain values, for comparing them with `toEqual`. */
export function assetValues(
  assets: readonly AssetFile[],
): readonly { readonly name: string; readonly bytes: readonly number[] }[] {
  return assets.map((asset) => ({ name: asset.name, bytes: Array.from(asset.bytes) }))
}
