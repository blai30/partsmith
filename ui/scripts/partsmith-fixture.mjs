// The name of Partsmith's source archive, the path of its script and the archive of a commit, which acceptance tests
// 57 and 117 to 120 of docs/spec/delivery.md section 6 share (section 2.13, Partsmith's source archive).
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'

/** The script that writes Partsmith's source archive, as a path from the repository root. */
export const PARTSMITH_SCRIPT = 'scripts/partsmith-source-archive.sh'

/** The name of Partsmith's source archive in every deployment, every release and every source artifact. */
export const PARTSMITH_ARCHIVE = 'partsmith-source.tar.gz'

/** The folder of every entry of the archive of `commit`, as GitHub's archive of a commit names it. */
export function archiveFolder(commit) {
  return `partsmith-${commit}/`
}

/**
 * Writes the archive of `commit` of the repository at `repository` to `path`, in the Git `environment`, as section
 * 2.13 makes it: `git archive` with the four `-c` options of the OCCT archive and the folder `partsmith-{commit}/`,
 * run with `GIT_ATTR_NOSYSTEM=1` and piped to `gzip -9 -n`. Fails the test when a command fails.
 */
export function writePartsmithArchive(environment, repository, commit, path) {
  const result = spawnSync(
    'bash',
    [
      '-c',
      'set -o pipefail; git -c core.autocrlf=false -c core.eol=lf -c tar.umask=0002 -c core.attributesFile=/dev/null archive --format=tar --prefix="$1" "$2" | gzip -9 -n > "$3"',
      'bash',
      archiveFolder(commit),
      commit,
      path,
    ],
    { cwd: repository, env: { ...environment, GIT_ATTR_NOSYSTEM: '1' }, encoding: 'utf8' },
  )
  assert.equal(result.status, 0, result.stderr)
}
