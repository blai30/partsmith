// The OCCT pins, the archive's name and the fixture repository that acceptance tests 55, 58, 86 and 87 of
// docs/spec/delivery.md section 6 share: a stand-in for OCCT whose tag names its first commit (section 2.13).
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { git } from './isolated-git.mjs'

const versionPath = fileURLToPath(new URL('../../scripts/occt-version.sh', import.meta.url))

// The text of `src/Standard/Standard.cxx` in the fixture's second commit, after the tag.
export const STANDARD_AFTER_TAG = '// Standard, after the tag\n'

/**
 * The `OCCT_VERSION`, `OCCT_TAG` and `OCCT_COMMIT` that scripts/occt-version.sh sets, as `{ version, tag, commit }`;
 * throws when it sets one of them to nothing.
 */
export async function readOcctPins() {
  const text = await readFile(versionPath, 'utf8')
  const value = (name) => {
    const match = new RegExp(`^${name}=(\\S+)$`, 'm').exec(text)
    if (match === null) {
      throw new Error(`scripts/occt-version.sh sets no ${name}`)
    }
    return match[1]
  }
  return { version: value('OCCT_VERSION'), tag: value('OCCT_TAG'), commit: value('OCCT_COMMIT') }
}

/** The text of an occt-version.sh that pins `version`, `tag` and `commit`. */
export function occtVersionText({ version, tag, commit }) {
  return `OCCT_VERSION=${version}\nOCCT_TAG=${tag}\nOCCT_COMMIT=${commit}\n`
}

/** The name of the OCCT source archive of OCCT `version`, as scripts/occt-source-archive.sh writes it (section 2.13). */
export function sourceArchiveName(version) {
  return `occt-${version}-source.tar.gz`
}

/** Writes `files`, a map from paths relative to `directory` to contents, creating their folders. */
export async function writeFiles(directory, files) {
  await Promise.all(
    Object.entries(files).map(async ([path, contents]) => {
      await mkdir(dirname(join(directory, path)), { recursive: true })
      await writeFile(join(directory, path), contents)
    }),
  )
}

/**
 * Makes a fixture repository at `directory` in the Git `environment`: a commit of `files` tagged `tag` (annotated,
 * with the message `OCCT {version}`), then a second commit that changes `src/Standard/Standard.cxx`, which `files`
 * must hold, to STANDARD_AFTER_TAG. Resolves to both commits as `{ tagged, second }`.
 */
export async function makeTaggedRepository(environment, directory, files, { version, tag }) {
  await mkdir(directory, { recursive: true })
  git(environment, directory, 'init', '--initial-branch=main')
  await writeFiles(directory, files)
  git(environment, directory, 'add', '--all')
  git(environment, directory, 'commit', '--message', 'OCCT')
  git(environment, directory, 'tag', '--annotate', tag, '--message', `OCCT ${version}`)
  const tagged = git(environment, directory, 'rev-parse', 'HEAD')
  await writeFile(join(directory, 'src/Standard/Standard.cxx'), STANDARD_AFTER_TAG)
  git(environment, directory, 'commit', '--all', '--message', 'After the tag')
  const second = git(environment, directory, 'rev-parse', 'HEAD')
  return { tagged, second }
}
