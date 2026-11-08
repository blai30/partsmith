import type { AssetKind } from '../generated/protocol'

// The asset files of a document's `assets/` folder and the files `addAssets` gives the core (ui.md
// 2.15, file-format.md 3.11). Since M2 the UI reads no `.ptsm` text: the core names the assets a
// document reads (`missing_assets`, ADR 0064).

/** A file of a document's `assets/` folder: its asset name and its bytes (file-format.md 3.11). */
export type AssetFile = { readonly name: string; readonly bytes: Uint8Array }

/** A file for `addAssets` (2.6): its kind, its bytes, and the name it was read under in the document's `assets/` folder, or null for a file the user picked. */
export type NewAsset = {
  readonly kind: AssetKind
  readonly bytes: Uint8Array
  readonly name: string | null
}

/** The asset kind of each lowercase file extension. */
const ASSET_KINDS: ReadonlyMap<string, AssetKind> = new Map([
  ['step', 'step'],
  ['stp', 'step'],
  ['ttf', 'ttf'],
  ['otf', 'otf'],
])

/** The asset kind of a file name by its extension, compared without case; null for any other name. */
export function assetKindOf(fileName: string): AssetKind | null {
  const dot = fileName.lastIndexOf('.')
  return dot < 0 ? null : (ASSET_KINDS.get(fileName.slice(dot + 1).toLowerCase()) ?? null)
}

/** Files read from a document's `assets/` folder as `addAssets` takes them: each with the kind of its name and that name; a name of no kind is left out. */
export function namedAssets(files: readonly AssetFile[]): readonly NewAsset[] {
  return files.flatMap((file) => {
    const kind = assetKindOf(file.name)
    return kind === null ? [] : [{ kind, bytes: file.bytes, name: file.name }]
  })
}
