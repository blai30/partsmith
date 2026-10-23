// The asset files of a document's `assets/` folder (ui.md 2.15, file-format.md 3.11) and the names
// Open reads from it (3.19 openFile step 3, ADR 0037 decision 2).

/** A file of a document's `assets/` folder: its asset name and its bytes (file-format.md 3.11). */
export type AssetFile = { readonly name: string; readonly bytes: Uint8Array }

/** An asset's file name (file-format.md 3.11): 64 lowercase hex digits, a dot and a lowercase extension. */
export const ASSET_NAME = /^[0-9a-f]{64}\.[a-z0-9]+$/

/**
 * The asset names among the string values anywhere in a document's JSON text, each once, sorted;
 * none when the text is not JSON. This is the one place the UI reads `.ptsm` text (ui.md section
 * 1): it collects matching strings and interprets nothing else, and object keys are not values.
 */
export function referencedAssetNames(text: string): readonly string[] {
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch {
    return []
  }
  const names = new Set<string>()
  const pending: unknown[] = [parsed]
  for (let value = pending.pop(); value !== undefined; value = pending.pop()) {
    if (typeof value === 'string') {
      if (ASSET_NAME.test(value)) {
        names.add(value)
      }
    } else if (typeof value === 'object' && value !== null) {
      // An array's values are its items.
      for (const child of Object.values(value)) {
        pending.push(child)
      }
    }
  }
  return [...names].toSorted()
}
