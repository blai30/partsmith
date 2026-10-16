// The Git environment of acceptance tests 52 and 55 of docs/spec/delivery.md section 6, so that the machine's Git
// configuration (an identity, commit signing, a default branch, a global attributes file) neither stops nor changes
// the repositories they make or the scripts they run.
import { spawnSync } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

// One fixed identity and time for every commit and tag, so the same steps make the same commits.
const IDENTITY = {
  name: 'Partsmith Tests',
  email: 'tests@partsmith.invalid',
  date: '2026-10-16T12:00:00Z',
}

/**
 * The environment of this process without its `GIT_` variables (configuration passed in the environment, or a
 * `GIT_DIR` from a hook), with system configuration off, an empty global configuration file and an empty
 * configuration folder made in `directory`, and the fixed identity.
 */
export async function isolatedGitEnvironment(directory) {
  const configurationFile = join(directory, 'git-configuration')
  const configurationFolder = join(directory, 'configuration-home')
  await writeFile(configurationFile, '')
  await mkdir(configurationFolder)
  const inherited = Object.entries(process.env).filter(([name]) => !name.startsWith('GIT_'))
  return {
    ...Object.fromEntries(inherited),
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: configurationFile,
    XDG_CONFIG_HOME: configurationFolder,
    GIT_AUTHOR_NAME: IDENTITY.name,
    GIT_AUTHOR_EMAIL: IDENTITY.email,
    GIT_AUTHOR_DATE: IDENTITY.date,
    GIT_COMMITTER_NAME: IDENTITY.name,
    GIT_COMMITTER_EMAIL: IDENTITY.email,
    GIT_COMMITTER_DATE: IDENTITY.date,
  }
}

/**
 * Runs `git` with `gitArguments` in `directory` and `environment`, and returns its standard output without the final
 * LF; throws when it fails.
 */
export function git(environment, directory, ...gitArguments) {
  const result = spawnSync('git', gitArguments, {
    cwd: directory,
    env: environment,
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    throw new Error(
      `git ${gitArguments.join(' ')} failed with exit status ${result.status}: ${result.stderr}`,
    )
  }
  return result.stdout.replace(/\n$/, '')
}
