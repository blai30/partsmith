// Acceptance tests 43 to 47 of docs/spec/delivery.md section 6 for the npm license check.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

import {
  EMITTED_BUILD_PACKAGES,
  allowedLicenses,
  licenseAllowed,
  licenseProblems,
  productionPackages,
  shippedPackages,
} from './check-licenses.mjs'

const scriptPath = fileURLToPath(new URL('check-licenses.mjs', import.meta.url))
const denyTomlPath = fileURLToPath(new URL('../../deny.toml', import.meta.url))
const uiDirectory = fileURLToPath(new URL('..', import.meta.url))

// The allow list of the repository's deny.toml, in order (section 2.10).
const repositoryLicenses = [
  'MIT',
  'Apache-2.0',
  'Apache-2.0 WITH LLVM-exception',
  'BSD-2-Clause',
  'BSD-3-Clause',
  'ISC',
  'Zlib',
  'Unicode-3.0',
  'CC0-1.0',
  'MPL-2.0',
  'BSL-1.0',
]

const denyTomlMessage =
  'deny.toml has no allow array of license strings in its [licenses] table (docs/spec/delivery.md section 2.10).'

const usageLine = 'usage: node scripts/check-licenses.mjs'

/** The section 4 message for a package that ships under a license the list does not allow. */
function notAllowedMessage(name, version, license) {
  return `npm package ${name} ${version} is licensed ${license}, which deny.toml does not allow. Replace the package, or allow the license in deny.toml with an ADR (docs/spec/delivery.md section 2.10).`
}

/** The section 4 message for a package of EMITTED_BUILD_PACKAGES that is not installed. */
function emittedPackageMessage(name) {
  return `npm package ${name} of EMITTED_BUILD_PACKAGES is not in pnpm licenses list --json. Remove it from the list if the build no longer uses it (docs/spec/delivery.md section 2.10).`
}

/** One entry of a `pnpm licenses list --json` group: a package with one path per version. */
function listed(name, versions, license) {
  return {
    name,
    versions,
    paths: versions.map((version) => `/store/${name}@${version}/node_modules/${name}`),
    license,
  }
}

/** The standard output of `pnpm licenses list --json` for `groups`, an object of license names to entries. */
function listing(groups) {
  return `${JSON.stringify(groups, null, 2)}\n`
}

/** A package as `productionPackages` returns it, with the path that `listed` gives. */
function shipped(name, version, license) {
  return { name, version, license, path: `/store/${name}@${version}/node_modules/${name}` }
}

/** Runs `pnpm` with `commandArguments` in ui/ and returns its standard output. */
function pnpmOutput(commandArguments) {
  const result = spawnSync('pnpm', commandArguments, { cwd: uiDirectory, encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
  return result.stdout
}

/** Runs `node scripts/check-licenses.mjs` with `commandArguments` in ui/. */
function runCommand(commandArguments) {
  const result = spawnSync(process.execPath, [scriptPath, ...commandArguments], {
    cwd: uiDirectory,
    encoding: 'utf8',
  })
  return { status: result.status, stdout: result.stdout, stderr: result.stderr }
}

test("43: allowedLicenses of the repository's deny.toml is the eleven identifiers, in order", async () => {
  assert.deepEqual(allowedLicenses(await readFile(denyTomlPath, 'utf8')), repositoryLicenses)
})

test('43: allowedLicenses reads an array with a comment line, a comment after an element and a trailing comma', () => {
  const denyToml = [
    '[licenses]',
    'confidence-threshold = 0.9',
    'allow = [',
    '  # Permissive licenses only.',
    '  "MIT", # the most common one',
    '  "ISC",',
    ']',
    '',
    '[bans]',
    'multiple-versions = "allow"',
    '',
  ].join('\n')

  assert.deepEqual(allowedLicenses(denyToml), ['MIT', 'ISC'])
})

test('43: allowedLicenses ignores an allow key in a table before [licenses]', () => {
  const denyToml = [
    '[bans]',
    'allow = ["GPL-3.0-only"]',
    '',
    '[licenses]',
    'allow = ["MIT"]',
    '',
  ].join('\n')

  assert.deepEqual(allowedLicenses(denyToml), ['MIT'])
})

test('43: allowedLicenses throws the deny.toml message for a text without [licenses]', () => {
  assert.throws(() => allowedLicenses('[bans]\nallow = ["MIT"]\n'), { message: denyTomlMessage })
})

test('43: allowedLicenses throws the deny.toml message for a [licenses] table without allow', () => {
  const denyToml = '[licenses]\nconfidence-threshold = 0.9\n\n[bans]\nallow = ["MIT"]\n'

  assert.throws(() => allowedLicenses(denyToml), { message: denyTomlMessage })
})

test('43: allowedLicenses throws the deny.toml message for an unterminated array', () => {
  assert.throws(() => allowedLicenses('[licenses]\nallow = [\n  "MIT",\n'), {
    message: denyTomlMessage,
  })
})

test('43: allowedLicenses throws the deny.toml message for an element without quotes', () => {
  assert.throws(() => allowedLicenses('[licenses]\nallow = [MIT]\n'), { message: denyTomlMessage })
})

test("44: licenseAllowed accepts expressions the repository's list satisfies", () => {
  const satisfied = [
    'MIT',
    'mit',
    '(MIT OR Apache-2.0)',
    'MIT OR GPL-3.0-only',
    'GPL-3.0-only OR MIT',
    'Apache-2.0 WITH LLVM-exception',
    'MIT AND (Apache-2.0 OR BSD-3-Clause)',
    'MIT OR GPL-3.0-only AND Zlib',
    '((ISC))',
  ]

  for (const expression of satisfied) {
    assert.equal(licenseAllowed(expression, repositoryLicenses), true, expression)
  }
})

test("44: licenseAllowed rejects expressions the repository's list does not satisfy and malformed ones", () => {
  const rejected = [
    'GPL-3.0-only',
    'MIT AND GPL-2.0-only',
    '(GPL-3.0-only OR MIT) AND BlueOak-1.0.0',
    'MIT WITH LLVM-exception',
    'GPL-2.0-only WITH Classpath-exception-2.0',
    'BlueOak-1.0.0',
    'UNLICENSED',
    'SEE LICENSE IN LICENSE.txt',
    'Unknown',
    '',
    'MIT OR',
    'MIT or Apache-2.0',
    '(MIT',
    'MIT)',
  ]

  for (const expression of rejected) {
    assert.equal(licenseAllowed(expression, repositoryLicenses), false, expression)
  }
})

test('45: productionPackages lists each version with its path and license, sorted by name and version', () => {
  const output = listing({
    MIT: [listed('a', ['1.0.0', '2.0.0'], 'MIT'), listed('c', ['0.1.0'], 'MIT')],
    ISC: [listed('b', ['1.0.0'], 'ISC')],
  })

  assert.deepEqual(productionPackages(output), [
    shipped('a', '1.0.0', 'MIT'),
    shipped('a', '2.0.0', 'MIT'),
    shipped('b', '1.0.0', 'ISC'),
    shipped('c', '0.1.0', 'MIT'),
  ])
})

test('45: productionPackages sorts by version precedence and lists a name and version given twice once', () => {
  const output = listing({
    MIT: [listed('a', ['10.0.0', '9.1.0', '9.1.0-beta.2'], 'MIT'), listed('a', ['9.1.0'], 'MIT')],
  })

  assert.deepEqual(productionPackages(output), [
    shipped('a', '9.1.0-beta.2', 'MIT'),
    shipped('a', '9.1.0', 'MIT'),
    shipped('a', '10.0.0', 'MIT'),
  ])
})

test('45: EMITTED_BUILD_PACKAGES is rolldown, tailwindcss and vite', () => {
  assert.deepEqual(EMITTED_BUILD_PACKAGES, ['rolldown', 'tailwindcss', 'vite'])
})

test('45: shippedPackages adds every version of each emitted build package to the production packages', () => {
  const production = listing({ MIT: [listed('react', ['19.3.0'], 'MIT')] })
  const all = listing({
    MIT: [
      listed('react', ['19.3.0'], 'MIT'),
      listed('vite', ['8.3.4', '7.0.0'], 'MIT'),
      listed('tailwindcss', ['4.3.3'], 'MIT'),
      listed('rolldown', ['1.2.13'], 'MIT'),
    ],
    'Apache-2.0': [listed('typescript', ['6.0.0'], 'Apache-2.0')],
  })

  assert.deepEqual(shippedPackages(production, all), [
    shipped('react', '19.3.0', 'MIT'),
    shipped('rolldown', '1.2.13', 'MIT'),
    shipped('tailwindcss', '4.3.3', 'MIT'),
    shipped('vite', '7.0.0', 'MIT'),
    shipped('vite', '8.3.4', 'MIT'),
  ])
})

test('45: shippedPackages throws the emitted-package message for the first name the full listing lacks', () => {
  const production = listing({ MIT: [listed('react', ['19.3.0'], 'MIT')] })
  const all = listing({
    MIT: [listed('react', ['19.3.0'], 'MIT'), listed('tailwindcss', ['4.3.3'], 'MIT')],
  })

  assert.throws(() => shippedPackages(production, all), {
    message: emittedPackageMessage('rolldown'),
  })
})

test('46: licenseProblems reports only the package whose license the list does not allow', () => {
  const packages = [
    shipped('react', '19.3.0', 'MIT'),
    shipped('lru-cache', '11.5.3', 'BlueOak-1.0.0'),
  ]

  assert.deepEqual(licenseProblems(packages, repositoryLicenses), [
    notAllowedMessage('lru-cache', '11.5.3', 'BlueOak-1.0.0'),
  ])
})

test('47: the command in ui/ exits 0 and counts the packages that ship', () => {
  const count = shippedPackages(
    pnpmOutput(['licenses', 'list', '--prod', '--json']),
    pnpmOutput(['licenses', 'list', '--json']),
  ).length

  assert.deepEqual(runCommand([]), {
    status: 0,
    stdout: `${count} npm packages that ship, each under a license deny.toml allows.\n`,
    stderr: '',
  })
})

test('47: the command with an argument exits 2 with the usage line', () => {
  assert.deepEqual(runCommand(['extra']), { status: 2, stdout: '', stderr: `${usageLine}\n` })
})
