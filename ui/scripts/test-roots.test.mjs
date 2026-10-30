// Acceptance test 80 of docs/spec/delivery.md section 6: every test file of a crate is declared by a test root (ADR 0035).
import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))

// The root of the `integration` target (ADR 0035): a line that a root lacks goes there, or into the first root without it.
const INTEGRATION_ROOT = 'integration.rs'

/** The patterns of the `members` list of the workspace manifest `manifest`, as written. */
function memberPatterns(manifest) {
  const list = /^members\s*=\s*\[([^\]]*)\]/m.exec(manifest)
  assert.ok(list, 'the workspace Cargo.toml has no members list')
  return [...list[1].matchAll(/"([^"]+)"/g)].map((match) => match[1])
}

/** The member folders that `pattern` names: the pattern itself, or for `<folder>/*` the folders in that folder. */
async function memberFolders(pattern) {
  const parent = pattern.endsWith('/*') ? pattern.slice(0, -2) : undefined
  assert.doesNotMatch(
    parent ?? pattern,
    /[*?[]/,
    `the member pattern ${pattern} is neither a folder nor <folder>/*`,
  )
  if (parent === undefined) {
    return [pattern]
  }
  const entries = await readdir(join(repositoryRoot, parent), { withFileTypes: true })
  return entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => `${parent}/${entry.name}`)
    .toSorted()
}

/**
 * What a member's manifest `manifest` says about its tests: whether Cargo makes a target of each file of `tests/`, as it
 * does unless `[package]` sets `autotests = false`, and the root of each `[[test]]` target as a path below `tests/`, its
 * `path` or else `<name>.rs`.
 */
function testTargets(manifest) {
  let table = ''
  let autotests = true
  const targets = []
  for (const line of manifest.split('\n').map((text) => text.trim())) {
    if (line.startsWith('[')) {
      table = line.replace(/\s*#.*$/, '')
      if (table === '[[test]]') {
        targets.push({})
      }
      continue
    }
    const entry = /^([\w-]+)\s*=\s*(?:"([^"]*)"|([^\s#]+))/.exec(line)
    if (entry === null) {
      continue
    }
    const key = entry[1]
    const value = entry[2] ?? entry[3]
    if (table === '[package]' && key === 'autotests') {
      autotests = value !== 'false'
    } else if (table === '[[test]]' && (key === 'name' || key === 'path')) {
      targets.at(-1)[key] = value
    }
  }
  return {
    autotests,
    roots: targets.map((target) => target.path?.replace(/^tests\//, '') ?? `${target.name}.rs`),
  }
}

/** The modules that the Rust source `source` declares from files: each line `mod <name>;`, public or not. */
function declaredModules(source) {
  const declarations = source.matchAll(/^\s*(?:pub(?:\([\w ]+\))?\s+)?mod\s+(\w+)\s*;/gm)
  return new Set([...declarations].map((declaration) => declaration[1]))
}

/** The module that the file at `path` below `tests/` is: its stem, or its folder's name for a `mod.rs`. */
function moduleName(path) {
  const parts = path.replace(/\.rs$/, '').split('/')
  return parts.at(-1) === 'mod' ? parts.at(-2) : parts.at(-1)
}

/**
 * The problems of ADR 0035 in the workspace member at the folder `member`, as lines to add: its manifest `manifest`
 * without `autotests = false` or without a `[[test]]` target, and each test file that no module tree reaches. `sources`
 * maps the path below `tests/` of every `.rs` file in `tests/` and in the folders directly in it to its text.
 */
function memberProblems(member, manifest, sources) {
  const { autotests, roots } = testTargets(manifest)
  const problems = []
  if (autotests) {
    problems.push(`${member}/Cargo.toml: add "autotests = false" to [package]`)
  }
  if (roots.length === 0) {
    problems.push(`${member}/Cargo.toml: add a [[test]] target with name = "integration"`)
  }
  const declaredBy = (file) => declaredModules(sources.get(file) ?? '')
  const namedRoot = roots.includes(INTEGRATION_ROOT)
    ? INTEGRATION_ROOT
    : (roots[0] ?? INTEGRATION_ROOT)
  const fromRoots = new Set(roots.flatMap((root) => [...declaredBy(root)]))
  const expectDeclared = (file, declaringFile, declared) => {
    const name = moduleName(file)
    if (!declared.has(name)) {
      problems.push(
        `${member}/tests/${file}: add "mod ${name};" to ${member}/tests/${declaringFile}`,
      )
    }
  }
  for (const file of [...sources.keys()].toSorted()) {
    const folder = file.includes('/') ? file.slice(0, file.indexOf('/')) : undefined
    if (folder === undefined || file === `${folder}/mod.rs`) {
      if (!roots.includes(file)) {
        expectDeclared(file, namedRoot, fromRoots)
      }
      continue
    }
    // A folder of modules holds a mod.rs or sits beside its parent module's file; any other folder holds none.
    const declaringFile = [`${folder}/mod.rs`, `${folder}.rs`].find((path) => sources.has(path))
    if (declaringFile !== undefined) {
      expectDeclared(file, declaringFile, declaredBy(declaringFile))
    }
  }
  return problems
}

/** The `.rs` files of the folder `tests` and of the folders directly in it, by their paths below `tests`, with their texts. */
async function testSources(tests) {
  const entries = await readdir(tests, { withFileTypes: true })
  const listed = await Promise.all(
    entries.map(async (entry) =>
      entry.isDirectory()
        ? (await readdir(join(tests, entry.name))).map((name) => `${entry.name}/${name}`)
        : [entry.name],
    ),
  )
  const paths = listed.flat().filter((path) => path.endsWith('.rs'))
  const texts = await Promise.all(paths.map((path) => readFile(join(tests, path), 'utf8')))
  return new Map(paths.map((path, i) => [path, texts[i]]))
}

/** The problems of the workspace member at the folder `member`, as `memberProblems` gives them from its files. */
async function problemsOf(member) {
  const folder = join(repositoryRoot, member)
  const [manifest, sources] = await Promise.all([
    readFile(join(folder, 'Cargo.toml'), 'utf8'),
    testSources(join(folder, 'tests')),
  ])
  return memberProblems(member, manifest, sources)
}

test('80: every test file of every workspace member is declared by a test root', async () => {
  const workspace = await readFile(join(repositoryRoot, 'Cargo.toml'), 'utf8')
  const members = (await Promise.all(memberPatterns(workspace).map(memberFolders))).flat()
  const tested = members.filter((member) => existsSync(join(repositoryRoot, member, 'tests')))
  const problems = (await Promise.all(tested.map(problemsOf))).flat()

  assert.ok(tested.length > 0, 'no workspace member has a tests folder')
  assert.deepEqual(
    problems,
    [],
    `files that no test binary builds (ADR 0035):\n${problems.join('\n')}`,
  )
})

// A member's manifest as ADR 0035 writes it, with an integration target and a budgets target.
const MANIFEST = `[package]
name = "partsmith-example"
# One test binary (ADR 0035).
autotests = false

[[test]]
name = "integration"

[[test]]
name = "budgets"

[dev-dependencies]
proptest = "1"
`

/** The problems of the member `crates/partsmith-example` with `MANIFEST` and the `.rs` files `files`, path and text. */
function exampleProblems(files, manifest = MANIFEST) {
  return memberProblems('crates/partsmith-example', manifest, new Map(Object.entries(files)))
}

test('80: a module declared under a cfg attribute counts as declared', () => {
  const files = {
    'integration.rs':
      'mod common;\n\n// Node only.\n#[cfg(not(target_family = "wasm"))]\nmod web_parity;\n',
    'common/mod.rs': 'pub mod block;\n',
    'common/block.rs': '',
    'web_parity.rs': '',
    'budgets.rs': '',
  }

  assert.deepEqual(exampleProblems(files), [])
})

test('80: a budgets target is a root, which nothing declares', () => {
  const files = { 'integration.rs': 'mod a;\n', 'a.rs': '', 'budgets.rs': 'mod a;\n' }
  const withoutBudgets = MANIFEST.replace('[[test]]\nname = "budgets"\n', '')

  assert.deepEqual(exampleProblems(files), [])
  assert.deepEqual(exampleProblems(files, withoutBudgets), [
    'crates/partsmith-example/tests/budgets.rs: add "mod budgets;" to crates/partsmith-example/tests/integration.rs',
  ])
})

test('80: a manifest without autotests = false gives one problem', () => {
  const files = { 'integration.rs': '', 'budgets.rs': '' }

  assert.deepEqual(exampleProblems(files, MANIFEST.replace('autotests = false\n', '')), [
    'crates/partsmith-example/Cargo.toml: add "autotests = false" to [package]',
  ])
})

test('80: a file in tests/ that no root declares gives one problem', () => {
  const files = {
    'integration.rs': 'mod common;\nmod a;\n',
    'a.rs': '',
    'b.rs': '',
    'budgets.rs': '',
    'common/mod.rs': '',
  }

  assert.deepEqual(exampleProblems(files), [
    'crates/partsmith-example/tests/b.rs: add "mod b;" to crates/partsmith-example/tests/integration.rs',
  ])
})

test('80: a file of a mod.rs folder that its mod.rs does not declare gives one problem', () => {
  const files = {
    'integration.rs': 'mod common;\n',
    'budgets.rs': '',
    'common/mod.rs': 'pub mod records;\n',
    'common/records.rs': '',
    'common/samples.rs': '',
  }

  assert.deepEqual(exampleProblems(files), [
    'crates/partsmith-example/tests/common/samples.rs: add "mod samples;" to crates/partsmith-example/tests/common/mod.rs',
  ])
})

test('80: a file of a folder beside its parent module that the module does not declare gives one problem', () => {
  const files = {
    'integration.rs': 'mod examples;\n',
    'budgets.rs': '',
    'examples.rs': 'mod export;\n',
    'examples/export.rs': '',
    'examples/coverage.rs': '',
  }

  assert.deepEqual(exampleProblems(files), [
    'crates/partsmith-example/tests/examples/coverage.rs: add "mod coverage;" to crates/partsmith-example/tests/examples.rs',
  ])
})
