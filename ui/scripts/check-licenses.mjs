// Checks that every npm package that ships is under a license the allow list of deny.toml allows
// (docs/spec/delivery.md section 2.10).
// Usage: node scripts/check-licenses.mjs
import { spawn } from 'node:child_process'
import { readFile, realpath } from 'node:fs/promises'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

/**
 * The development dependencies whose code the build writes into `ui/dist`: Rolldown's CommonJS interop helpers at the
 * top of `assets/index-*.js`, Vite's module preload polyfill (test 32), and Tailwind's CSS with its license banner.
 */
export const EMITTED_BUILD_PACKAGES = ['rolldown', 'tailwindcss', 'vite']

const USAGE = 'usage: node scripts/check-licenses.mjs'

const DENY_TOML_MESSAGE =
  'deny.toml has no allow array of license strings in its [licenses] table (docs/spec/delivery.md section 2.10).'

const denyTomlPath = fileURLToPath(new URL('../../deny.toml', import.meta.url))
const uiDirectory = fileURLToPath(new URL('..', import.meta.url))

// The header line of the [licenses] table.
const LICENSES_HEADER_PATTERN = /^[ \t]*\[[ \t]*licenses[ \t]*\][ \t]*(?:#.*)?$/

// The header line of any table or array of tables, which ends the table before it.
const TABLE_HEADER_PATTERN = /^[ \t]*\[\[?[ \t]*[A-Za-z0-9_.-]+[ \t]*\]\]?[ \t]*(?:#.*)?$/

// The `allow` key of a table and the spaces up to its value.
const ALLOW_KEY_PATTERN = /^[ \t]*allow[ \t]*=[ \t]*/m

// An SPDX license or exception identifier.
const IDENTIFIER_PATTERN = /^[A-Za-z0-9.+-]+$/

// A version as Semantic Versioning 2.0.0 writes it, with its numbers and pre-release identifiers captured.
const VERSION_PATTERN = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/

/** Orders two strings by code units, so the order is the same in every locale; the notices sort paths with it. */
export function compareText(first, second) {
  if (first === second) {
    return 0
  }
  return first < second ? -1 : 1
}

/** Orders two strings of decimal digits without leading zeros by their numeric value. */
function compareNumerals(first, second) {
  return first.length - second.length || compareText(first, second)
}

/** Orders two pre-release identifiers: numbers by value, below any alphanumeric identifier. */
function compareIdentifiers(first, second) {
  const firstIsNumber = /^\d+$/.test(first)
  const secondIsNumber = /^\d+$/.test(second)
  if (firstIsNumber && secondIsNumber) {
    return compareNumerals(first, second)
  }
  if (firstIsNumber !== secondIsNumber) {
    return firstIsNumber ? -1 : 1
  }
  return compareText(first, second)
}

/** Orders two pre-release parts; a version without one (undefined) comes after any version with one. */
function comparePreReleases(first, second) {
  if (first === undefined || second === undefined) {
    return (first === undefined) - (second === undefined)
  }
  const firstIdentifiers = first.split('.')
  const secondIdentifiers = second.split('.')
  for (let i = 0; i < Math.min(firstIdentifiers.length, secondIdentifiers.length); i += 1) {
    const order = compareIdentifiers(firstIdentifiers[i], secondIdentifiers[i])
    if (order !== 0) {
      return order
    }
  }
  return firstIdentifiers.length - secondIdentifiers.length
}

/**
 * Orders two versions by Semantic Versioning 2.0.0 precedence. Versions of equal precedence (or that are not semantic
 * versions) fall back to code unit order, so the order is total.
 */
function compareVersions(first, second) {
  const firstParts = VERSION_PATTERN.exec(first)
  const secondParts = VERSION_PATTERN.exec(second)
  if (firstParts === null || secondParts === null) {
    return compareText(first, second)
  }
  for (let i = 1; i <= 3; i += 1) {
    const order = compareNumerals(firstParts[i], secondParts[i])
    if (order !== 0) {
      return order
    }
  }
  return comparePreReleases(firstParts[4], secondParts[4]) || compareText(first, second)
}

/**
 * Orders two `{ name, version }` records by name (code unit order) and then by version precedence (Semantic
 * Versioning 2.0.0), the order of the npm packages here and of the crates of the notices (section 2.9).
 */
export function compareNameAndVersion(first, second) {
  return compareText(first.name, second.name) || compareVersions(first.version, second.version)
}

/**
 * Runs `command` with `commandArguments` in `directory` and resolves to its standard output. A command that cannot
 * start or exits with another status than 0 rejects with an `Error` whose `status` is the exit status (or the reason
 * it did not start) and whose `stderr` is its standard error.
 */
export function runCommand(command, commandArguments, directory) {
  return new Promise((resolve, reject) => {
    // On Windows, pnpm is a .cmd script, which only a shell starts.
    const child = spawn(command, commandArguments, {
      cwd: directory,
      shell: process.platform === 'win32',
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    const output = []
    const errorOutput = []
    child.stdout.on('data', (chunk) => output.push(chunk))
    child.stderr.on('data', (chunk) => errorOutput.push(chunk))
    child.on('error', (error) => {
      reject(Object.assign(error, { status: error.code, stderr: error.message }))
    })
    child.on('close', (status, signal) => {
      if (status === 0) {
        resolve(Buffer.concat(output).toString('utf8'))
        return
      }
      const failure = new Error(`${command} exited with status ${status ?? signal}`)
      reject(
        Object.assign(failure, {
          status: status ?? signal,
          stderr: Buffer.concat(errorOutput).toString('utf8'),
        }),
      )
    })
  })
}

/** The section 4 message for a command that `runCommand` rejected with `error`. */
export function commandFailureMessage(command, commandArguments, error) {
  return `${[command, ...commandArguments].join(' ')} failed with exit status ${error.status}: ${String(error.stderr ?? '').trim()}`
}

/** The lines of the `[licenses]` table of `denyToml`, from its header to the next table header. */
function licensesTable(denyToml) {
  const lines = denyToml.split(/\r?\n/)
  const start = lines.findIndex((line) => LICENSES_HEADER_PATTERN.test(line))
  if (start === -1) {
    throw new Error(DENY_TOML_MESSAGE)
  }
  const following = lines.slice(start + 1)
  const end = following.findIndex((line) => TABLE_HEADER_PATTERN.test(line))
  return (end === -1 ? following : following.slice(0, end)).join('\n')
}

/** The strings of the array that starts at `start` in `text`: double-quoted strings, comments and one trailing comma. */
function stringArray(text, start) {
  if (text[start] !== '[') {
    throw new Error(DENY_TOML_MESSAGE)
  }
  const strings = []
  let expectsString = true
  let index = start + 1
  while (index < text.length) {
    const character = text[index]
    if (/\s/.test(character)) {
      index += 1
    } else if (character === '#') {
      const lineEnd = text.indexOf('\n', index)
      index = lineEnd === -1 ? text.length : lineEnd
    } else if (character === ']') {
      return strings
    } else if (character === ',' && !expectsString) {
      expectsString = true
      index += 1
    } else if (character === '"' && expectsString) {
      const end = text.indexOf('"', index + 1)
      const value = end === -1 ? undefined : text.slice(index + 1, end)
      if (value === undefined || /[\n\\]/.test(value)) {
        throw new Error(DENY_TOML_MESSAGE)
      }
      strings.push(value)
      expectsString = false
      index = end + 1
    } else {
      throw new Error(DENY_TOML_MESSAGE)
    }
  }
  throw new Error(DENY_TOML_MESSAGE)
}

/** The strings of the `allow` array of the `[licenses]` table of `denyToml` (the text of deny.toml), in order. */
export function allowedLicenses(denyToml) {
  const table = licensesTable(denyToml)
  const key = ALLOW_KEY_PATTERN.exec(table)
  if (key === null) {
    throw new Error(DENY_TOML_MESSAGE)
  }
  return stringArray(table, key.index + key[0].length)
}

/** A syntax error in a license expression, which makes it unsatisfied. */
class ExpressionError extends Error {}

/** The tokens of a license expression: parentheses, operators and identifiers. */
function tokensOf(expression) {
  return expression.replaceAll(/[()]/g, ' $& ').split(/\s+/).filter(Boolean)
}

/** Whether the SPDX license expression `expression` is satisfied by the identifiers `allowed` (section 2.10). */
export function licenseAllowed(expression, allowed) {
  const names = new Set(allowed.map((license) => license.split(/\s+/).join(' ').toLowerCase()))
  const tokens = tokensOf(expression)
  let position = 0

  function identifier() {
    const token = tokens[position]
    if (
      token === undefined ||
      ['AND', 'OR', 'WITH'].includes(token) ||
      !IDENTIFIER_PATTERN.test(token)
    ) {
      throw new ExpressionError(expression)
    }
    position += 1
    return token
  }

  function term() {
    if (tokens[position] === '(') {
      position += 1
      const satisfied = disjunction()
      if (tokens[position] !== ')') {
        throw new ExpressionError(expression)
      }
      position += 1
      return satisfied
    }
    const license = identifier()
    if (tokens[position] !== 'WITH') {
      return names.has(license.toLowerCase())
    }
    position += 1
    return names.has(`${license} WITH ${identifier()}`.toLowerCase())
  }

  function conjunction() {
    let satisfied = term()
    while (tokens[position] === 'AND') {
      position += 1
      const next = term()
      satisfied &&= next
    }
    return satisfied
  }

  function disjunction() {
    let satisfied = conjunction()
    while (tokens[position] === 'OR') {
      position += 1
      const next = conjunction()
      satisfied ||= next
    }
    return satisfied
  }

  try {
    const satisfied = disjunction()
    return position === tokens.length && satisfied
  } catch (error) {
    if (error instanceof ExpressionError) {
      return false
    }
    throw error
  }
}

/** The packages of `packages` with each name and version once, the first of each kept, sorted by name and version. */
function uniquePackages(packages) {
  const byKey = new Map()
  for (const listed of packages) {
    const key = `${listed.name} ${listed.version}`
    if (!byKey.has(key)) {
      byKey.set(key, listed)
    }
  }
  return [...byKey.values()].toSorted(compareNameAndVersion)
}

/**
 * The packages of `listing`, the standard output of `pnpm licenses list --json` with or without `--prod`, as
 * `{ name, version, license, path }`, one per name and version, sorted by name and then by version precedence.
 */
export function productionPackages(listing) {
  const groups = Object.entries(JSON.parse(listing))
  return uniquePackages(
    groups.flatMap(([group, entries]) =>
      entries.flatMap((entry) =>
        entry.versions.map((version, i) => ({
          name: entry.name,
          version,
          license: entry.license ?? group,
          path: entry.paths[i] ?? entry.paths[0],
        })),
      ),
    ),
  )
}

/**
 * The packages that ship: `productionPackages(production)` and, from `productionPackages(all)`, every version of each
 * name of EMITTED_BUILD_PACKAGES, each name and version once, sorted as `productionPackages` sorts. `production` and
 * `all` are the standard outputs of `pnpm licenses list --prod --json` and `pnpm licenses list --json`. Throws an
 * `Error` with the emitted-package message of section 4 for the first name, in list order, that `all` lacks.
 */
export function shippedPackages(production, all) {
  const everyPackage = productionPackages(all)
  const emitted = EMITTED_BUILD_PACKAGES.flatMap((name) => {
    const versions = everyPackage.filter((listed) => listed.name === name)
    if (versions.length === 0) {
      throw new Error(
        `npm package ${name} of EMITTED_BUILD_PACKAGES is not in pnpm licenses list --json. Remove it from the list if the build no longer uses it (docs/spec/delivery.md section 2.10).`,
      )
    }
    return versions
  })
  return uniquePackages([...productionPackages(production), ...emitted])
}

/** One message (section 4) for each package whose license `licenseAllowed` rejects, in the order of `packages`. */
export function licenseProblems(packages, allowed) {
  return packages
    .filter((listed) => !licenseAllowed(listed.license, allowed))
    .map(
      (listed) =>
        `npm package ${listed.name} ${listed.version} is licensed ${listed.license}, which deny.toml does not allow. Replace the package, or allow the license in deny.toml with an ADR (docs/spec/delivery.md section 2.10).`,
    )
}

/** The standard output of `pnpm licenses list` with `commandArguments` in ui/, or the failure message of section 4. */
async function pnpmListing(commandArguments) {
  const pnpmArguments = ['licenses', 'list', ...commandArguments]
  try {
    return await runCommand('pnpm', pnpmArguments, uiDirectory)
  } catch (error) {
    throw new Error(commandFailureMessage('pnpm', pnpmArguments, error), { cause: error })
  }
}

/** Runs the command with its arguments and returns its exit status. */
async function main(commandArguments) {
  if (commandArguments.length > 0) {
    process.stderr.write(`${USAGE}\n`)
    return 2
  }
  try {
    const allowed = allowedLicenses(await readFile(denyTomlPath, 'utf8'))
    const packages = shippedPackages(
      await pnpmListing(['--prod', '--json']),
      await pnpmListing(['--json']),
    )
    const problems = licenseProblems(packages, allowed)
    if (problems.length > 0) {
      process.stderr.write(problems.map((problem) => `${problem}\n`).join(''))
      return 1
    }
    process.stdout.write(
      `${packages.length} npm packages that ship, each under a license deny.toml allows.\n`,
    )
    return 0
  } catch (error) {
    process.stderr.write(`${error.message}\n`)
    return 1
  }
}

// Runs as a command only when Node started this file, not when another module imports it.
const startedFile = process.argv[1]
if (startedFile !== undefined && (await realpath(startedFile)) === fileURLToPath(import.meta.url)) {
  process.exitCode = await main(process.argv.slice(2))
}
