type FileSystem = { readFileSync(path: URL, encoding: 'utf8'): string }

/**
 * The text of the file at `path` in Vite's project root, the ui folder, read through Node, which runs Vitest. `path`
 * starts with `/`, like the keys of `import.meta.glob`. The app's types leave out Node's, so `node:fs` is imported by
 * a specifier TypeScript does not follow.
 */
export async function readProjectFile(path: string): Promise<string> {
  const specifier: string = 'node:fs'
  const fileSystem = (await import(/* @vite-ignore */ specifier)) as FileSystem
  // A variable, because Vite rewrites a template literal in new URL(..., import.meta.url) into a lookup of asset files.
  const relativePath = `../..${path}`
  return fileSystem.readFileSync(new URL(relativePath, import.meta.url), 'utf8')
}
