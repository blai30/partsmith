type FileSystem = {
  readFileSync(path: URL, encoding: 'utf8'): string
  readFileSync(path: URL): Uint8Array
  readdirSync(path: URL): string[]
}

type Hash = { update(data: Uint8Array): Hash; digest(encoding: 'hex'): string }
type Crypto = { createHash(algorithm: 'sha256'): Hash }

// The app's types leave out Node's, so Node modules are imported by a specifier TypeScript does not follow.
async function nodeModule<Module>(specifier: string): Promise<Module> {
  return (await import(/* @vite-ignore */ specifier)) as Module
}

function fileSystem(): Promise<FileSystem> {
  return nodeModule<FileSystem>('node:fs')
}

function projectUrl(path: string): URL {
  // A variable, because Vite rewrites a template literal in new URL(..., import.meta.url) into a lookup of asset files.
  const relativePath = `../..${path}`
  return new URL(relativePath, import.meta.url)
}

/**
 * The text of the file at `path` in Vite's project root, the ui folder, read through Node, which runs Vitest. `path`
 * starts with `/`, like the keys of `import.meta.glob`; one that starts with `/..` reaches the repository above the ui
 * folder (U-247 reads `examples/` that way).
 */
export async function readProjectFile(path: string): Promise<string> {
  return (await fileSystem()).readFileSync(projectUrl(path), 'utf8')
}

/** The bytes of the file at `path` in the ui folder, read as `readProjectFile` reads text. */
export async function readProjectBytes(path: string): Promise<Uint8Array> {
  return (await fileSystem()).readFileSync(projectUrl(path))
}

/** The names of the entries of the folder at `path` in the ui folder, sorted. */
export async function listProjectFolder(path: string): Promise<string[]> {
  return (await fileSystem()).readdirSync(projectUrl(path)).toSorted()
}

/** The SHA-256 of the file at `path` in the ui folder, in hex. */
export async function projectFileSha256(path: string): Promise<string> {
  const crypto = await nodeModule<Crypto>('node:crypto')
  return crypto
    .createHash('sha256')
    .update(await readProjectBytes(path))
    .digest('hex')
}
