// Acceptance tests 49 to 54, 57, 59 to 63, 66, 67, 73, 79, 81 to 83, 85 to 87, 93 to 95, 100, 101, 104, 105, 118 to
// 120, 122 to 124, 128 and 130 of docs/spec/delivery.md section 6 for the workflow files, the copies of section 3.6,
// scripts/check-source-archive.sh and scripts/check-windows-imports.sh.
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  writeFile,
} from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { isDeepStrictEqual } from 'node:util'
import { gunzipSync, gzipSync } from 'node:zlib'

import { git, isolatedGitEnvironment } from './isolated-git.mjs'
import {
  makeTaggedRepository,
  occtVersionText,
  readOcctPins,
  sourceArchiveName,
  writeFiles,
} from './occt-fixture.mjs'
import { PARTSMITH_ARCHIVE, PARTSMITH_SCRIPT, writePartsmithArchive } from './partsmith-fixture.mjs'

const repositoryRoot = fileURLToPath(new URL('../..', import.meta.url))
const workflowsDirectory = join(repositoryRoot, '.github/workflows')

// The source stamp at the root of every web build, which tests 52, 53 and 57 name (section 2.13).
const SOURCE_STAMP = 'SOURCE.txt'

// The script that writes the OCCT source archive of a release and of every web deployment, which tests 54, 57 and 85
// name (section 2.13).
const ARCHIVE_SCRIPT = 'scripts/occt-source-archive.sh'

// The pins of OCCT that the archive's name and the fixtures of tests 86 and 87 read (sections 2.9 and 2.13).
const OCCT_PINS = 'scripts/occt-version.sh'

// The cache of the OCCT source archive (section 2.13): its folder, the key under which ci.yml's sources job restores it
// and saves it, and the two lines of the step that makes the archive on a miss, which release.yml's sources job runs in
// every run.
const ARCHIVE_CACHE_PATH = '.occt/source'
const ARCHIVE_CACHE_KEY =
  "occt-source-${{ hashFiles('scripts/occt-source-archive.sh', 'scripts/occt-version.sh') }}"
const ARCHIVE_STEP_RUN = `mkdir -p .occt/source\n${ARCHIVE_SCRIPT} .occt/source\n`

// The steps of section 2.13 that write the archive into the source artifact (sources) and check it before Wrangler
// (deploy).
const ARCHIVE_WRITER = 'Write the OCCT source archive'
const ARCHIVE_CHECK = 'Check the OCCT source archive'

// The script that checks a source archive, which those steps and the Partsmith check run (section 2.13, The archive
// check, tests 86, 87, 120 and 122), and its usage line (section 4).
const CHECK_SCRIPT = 'scripts/check-source-archive.sh'
const CHECK_USAGE = `usage: ${CHECK_SCRIPT} <archive> <commit>`

// The script that writes the dependency source archives, and the steps of section 2.13 that write them into the source
// artifact (sources) and check them before Wrangler (deploy), with the exact runs of the web and desktop scopes (tests
// 93 to 95).
const DEPENDENCY_SCRIPT = 'scripts/dependency-source-archive.mjs'
const DEPENDENCY_WRITER = 'Write the dependency source archives'
const DEPENDENCY_CHECK = 'Check the dependency source archives'
const DEPENDENCY_WRITER_RUN = [
  'mkdir sources/dependencies',
  `node ${DEPENDENCY_SCRIPT} web sources/dependencies`,
  'cd sources/dependencies',
  'export LC_ALL=C',
  'sha256sum -- * > SHA256SUMS',
  '',
].join('\n')
const DEPENDENCY_RELEASE_RUN = `node ${DEPENDENCY_SCRIPT} desktop sources`

// The job of release.yml that wrote the desktop scope's archives in a dry run before wave G, which the release's
// sources job replaces (section 2.5, test 94).
const DEPENDENCY_DRY_RUN = 'dependency-sources'

// The job of each workflow that writes the source artifact, its name, the artifact, the folder its steps write and the
// step that writes Partsmith's source archive there, with its exact run (section 2.13, The source with every artifact,
// tests 118 to 120).
const SOURCES_JOB = 'sources'
const SOURCES_FOLDER = 'sources'
const SOURCE_ARTIFACTS = {
  'ci.yml': { name: 'Source archives (web)', artifact: 'sources-web' },
  'release.yml': { name: 'Source archives (desktop)', artifact: 'sources-desktop' },
}

// The workflow files that tests 85, 118 and 122 hold to the source rules of section 2.13: the two with a sources job,
// and platforms.yml, which has had none since its web jobs moved into ci.yml (section 2.14, The move).
const SOURCE_RULE_FILES = [...Object.keys(SOURCE_ARTIFACTS), 'platforms.yml']
const PARTSMITH_WRITER = 'Write the Partsmith source archive'
const PARTSMITH_WRITER_RUN = `${PARTSMITH_SCRIPT} ${SOURCES_FOLDER}`

// The `if` of release.yml's sources job and of its bundle job (section 2.5).
const RELEASE_SOURCES_CONDITION =
  "${{ !cancelled() && (needs.verify.result == 'success' || needs.verify.result == 'skipped') }}"
const BUNDLE_CONDITION = "${{ !cancelled() && needs.sources.result == 'success' }}"

// The days each source artifact is kept, one more than every other artifact's 7 (section 2.13, Kept a day longer).
const SOURCE_RETENTION_DAYS = '8'
const ARTIFACT_RETENTION_DAYS = '7'

// The step of the deploy job that checks Partsmith's source archive, and its exact run (sections 2.4 and 2.13).
const PARTSMITH_CHECK = 'Check the Partsmith source archive'
const PARTSMITH_CHECK_RUN = [
  `archive=ui/dist/${PARTSMITH_ARCHIVE}`,
  `if ! ${CHECK_SCRIPT} "$archive" "$GITHUB_SHA"; then`,
  '  echo "::error title=Partsmith source missing::$archive is missing or does not hold the source of the commit $GITHUB_SHA, so nothing was deployed. The sources job writes it, and the web job puts it into the build (docs/spec/delivery.md section 2.13)."',
  '  exit 1',
  'fi',
  '',
].join('\n')

// The step that installs the toolchain of rust-toolchain.toml in the jobs that run cargo (sections 2.4 and 2.13).
const TOOLCHAIN_STEP = 'rustup show active-toolchain || rustup toolchain install'

// The lint and format scripts of ui/package.json, which also reach the dependency script from the repository root
// (test 94).
const LINT_SCRIPT = `oxlint --deny-warnings && oxfmt --check && cd .. && oxlint --deny-warnings -c ui/.oxlintrc.json ${DEPENDENCY_SCRIPT} && oxfmt --check -c ui/.oxfmtrc.json ${DEPENDENCY_SCRIPT}`
const FORMAT_SCRIPT = `oxfmt && cd .. && oxfmt -c ui/.oxfmtrc.json ${DEPENDENCY_SCRIPT}`

// The parts of the web archives that the stand-in for the script writes (test 93), in byte order of their names.
const WEB_PARTS = [
  'npm-packages-web.tar.part-00',
  'npm-packages-web.tar.part-01',
  'rust-crates-web.tar.gz.part-00',
]

// The Measured paragraph of section 2.13: the tag and commit it measured, and the archive's SHA-256 (test 86).
const MEASURED_ARCHIVE =
  /^\*\*Measured\*\* \([^)]*?the tag `([^`]+)`, commit `([0-9a-f]{40})`.*?SHA-256 `([0-9a-f]{64})`/m

// The size of big.bin in the fixtures of tests 86, 87 and 120: its tar is larger than a pipe's buffer (64 KiB on
// Linux). gzip is then still writing when git get-tar-commit-id has read the first header and exits.
const PIPE_BREAKING_SIZE = 1_048_576

// An OCCT version that no pin names, for an archive the steps must not take for the pinned one (tests 86 and 87).
const OTHER_OCCT_VERSION = '9.9.9'

// The repository and a pull request as GitHub gives them to the source stamp's writer (section 2.13).
const REPOSITORY_URL = 'https://github.com/blai30/partsmith'
const PULL_REQUEST_URL = 'https://github.com/blai30/partsmith/pull/7'

// Two commits, 40 hexadecimal digits as GitHub writes them: the head of main that a night tests, and an older one.
const HEAD_COMMIT = 'a'.repeat(40)
const OTHER_COMMIT = 'b'.repeat(40)

// The arguments of the skip check's gh run list (section 2.11), one per element.
const SKIP_CHECK_ARGUMENTS = [
  'run',
  'list',
  '--workflow',
  'nightly.yml',
  '--branch',
  'main',
  '--status',
  'completed',
  '--limit',
  '1',
  '--json',
  'headSha',
  '--jq',
  '.[0].headSha // ""',
]

// The timeout-minutes of every job of each workflow file, in the order of its jobs (sections 2.4, 2.5, 2.11 and 2.14).
const EXPECTED_TIMEOUTS = {
  'ci.yml': {
    'occt-native': 150,
    'occt-wasm': 150,
    'kernel-wasm': 60,
    rust: 60,
    deny: 60,
    ui: 60,
    schema: 60,
    sources: 60,
    'core-wasm': 60,
    web: 60,
    e2e: 60,
    python: 60,
    desktop: 60,
    'occt-platforms': 150,
    'rust-platforms': 60,
    'e2e-engines': 60,
    deploy: 60,
  },
  'release.yml': { verify: 60, sources: 60, bundle: 150, draft: 60 },
  'nightly.yml': { properties: 20 },
  'platforms.yml': { 'occt-platforms': 150, 'full-rebuild': 60 },
}

// The artifacts each workflow file uploads, in the order of its steps (sections 2.4, 2.5 and 2.14): its source artifact
// first, kept 8 days, and the others, kept 7 (section 2.13). platforms.yml uploads none since the move (section 2.14).
const EXPECTED_UPLOADS = {
  'ci.yml': [
    'sources-web',
    'core-wasm',
    'web-dist',
    'playwright-report',
    'playwright-report-${{ matrix.browser }}',
  ],
  'release.yml': ['sources-desktop', 'bundle-${{ runner.os }}'],
  'platforms.yml': [],
}

// The jobs of ci.yml that download core-wasm into ui/public (sections 2.4 and 2.14).
const CORE_DOWNLOADS = {
  'ci.yml': ['web', 'e2e', 'e2e-engines'],
}

// The pinned actions of section 2.2, each with its exact release.
const PINNED_ACTIONS = {
  'actions/checkout': 'v7.0.1',
  'actions/cache': 'v6.1.0',
  'actions/cache/restore': 'v6.1.0',
  'actions/cache/save': 'v6.1.0',
  'actions/setup-node': 'v7.1.0',
  'pnpm/action-setup': 'v6.1.0',
  'Swatinem/rust-cache': 'v2.9.2',
  'EmbarkStudios/cargo-deny-action': 'v2.1.1',
  'mymindstorm/setup-emsdk': 'v16',
  'actions/upload-artifact': 'v7.0.2',
  'actions/download-artifact': 'v8.0.2',
  'actions/setup-python': 'v7.0.0',
  'cloudflare/wrangler-action': 'v4.1.3',
  'ilammy/msvc-dev-cmd': 'v1.13.0',
}

// The workflow files of section 2.1.
const WORKFLOW_FILES = ['ci.yml', 'release.yml', 'nightly.yml', 'platforms.yml']

// The jobs of ci.yml that run on Windows and macOS (section 2.14), and rust-platforms' three test steps: the crate
// tests, then the examples runner and clippy, which also run after failing crate tests.
const PLATFORM_JOBS = ['occt-platforms', 'rust-platforms']
const CRATE_TESTS =
  'cargo test --no-fail-fast -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop'
const EXAMPLES_RUNNER = 'cargo test -p partsmith-cli --test integration examples::\n'
const PLATFORM_CLIPPY =
  'cargo clippy -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop --all-targets -- -D warnings'
const AFTER_CRATE_TESTS = "${{ !cancelled() && steps.crates.outcome != 'skipped' }}"

// The jobs that moved from platforms.yml into ci.yml in wave H, in their order there, each with its name, the name of
// the checks that branch protection requires (section 2.14, The move, test 128).
const MOVED_JOBS = {
  'occt-platforms': 'OCCT (${{ matrix.runner }})',
  'rust-platforms': 'Rust tests (${{ matrix.runner }})',
  'e2e-engines': 'End-to-end tests (${{ matrix.browser }})',
}

// The two lines in which platforms.yml's copy of the macos-15 row of occt-platforms differs from ci.yml's job: its name
// (choice 3) and its matrix (test 128).
const COPY_NAME_LINE = '    name: OCCT for the full rebuild (${{ matrix.runner }})'
const COPY_MATRIX_LINE = '        runner: [macos-15]'

// The jobs that platforms.yml dropped with the move (test 128).
const DROPPED_PLATFORMS_JOBS = [
  'rust-platforms',
  'e2e-engines',
  'occt-wasm',
  'sources',
  'core-wasm',
]

// The needs of ci.yml's deploy, every job that gates it in order (section 2.4, test 60).
const DEPLOY_NEEDS = [
  'kernel-wasm',
  'rust',
  'deny',
  'ui',
  'schema',
  'web',
  'e2e',
  'python',
  'desktop',
  'occt-platforms',
  'rust-platforms',
  'e2e-engines',
]

// The step of rust-platforms and of the release's bundle job that checks the imports of the Windows app, the condition
// it runs on in rust-platforms, the app each job checks, and the command of the bundle job that it follows (section
// 3.6, The check of the imports).
const IMPORT_CHECK = 'Check the imports of the app'
const IMPORT_CHECK_CONDITION =
  "${{ !cancelled() && steps.crates.outcome != 'skipped' && runner.os == 'Windows' }}"
const DEBUG_APP = 'target/debug/partsmith-desktop.exe'
const RELEASE_APP = 'target/release/partsmith-desktop.exe'
const TAURI_BUILD = 'cargo tauri build --bundles ${{ matrix.bundles }} --ci'

// The step of rust-platforms that checks the imports of the CLI, the CLI that Cargo builds for the examples runner, and
// the step that follows the check (section 3.6, The check of the imports, test 124).
const CLI_IMPORT_CHECK = 'Check the imports of the CLI'
const DEBUG_CLI = 'target/debug/partsmith.exe'
const DISK_SPACE = 'Disk space after the tests'

// The script that each import check runs, and its usage line (section 3.6, The check of the imports, and section 4).
const IMPORT_SCRIPT = 'scripts/check-windows-imports.sh'
const IMPORT_USAGE = `usage: ${IMPORT_SCRIPT} <executable>`

/** The `run` of an import check of the program `executable`: the import script with that one argument. */
function importCheckRun(executable) {
  return `${IMPORT_SCRIPT} ${executable}`
}

// A stand-in for dumpbin that appends its arguments, separated by NUL, as one line to $DUMPBIN_CALLS, prints the file
// $DUMPBIN_LISTING and exits with $DUMPBIN_STATUS.
const DUMPBIN_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$DUMPBIN_CALLS"
printf '\\n' >> "$DUMPBIN_CALLS"
cat "$DUMPBIN_LISTING"
exit "$DUMPBIN_STATUS"
`

// DLLs of Windows that hold no C runtime, as dumpbin names them (tests 101 and 123).
const SYSTEM_IMPORTS = ['KERNEL32.dll', 'USER32.dll', 'ole32.dll']

// The imports of an app that takes its C runtime from Windows alone, as dumpbin names them (tests 101 and 123).
const WINDOWS_IMPORTS = [
  ...SYSTEM_IMPORTS,
  'api-ms-win-crt-runtime-l1-1-0.dll',
  'api-ms-win-crt-heap-l1-1-0.dll',
]

// API sets of Windows' core, which the debug app imports and which are not the Universal C Runtime (test 123).
const CORE_API_SETS = ['api-ms-win-core-synch-l1-2-0.dll', 'api-ms-win-core-winrt-error-l1-1-0.dll']

// The dependencies dumpbin listed for the debug app in platforms run 14 (41926898084, job 126946230717), in order
// (test 123).
const RUN_14_APP_IMPORTS = [
  'bcryptprimitives.dll',
  'advapi32.dll',
  'ntdll.dll',
  'kernel32.dll',
  'shlwapi.dll',
  'comctl32.dll',
  'ole32.dll',
  'api-ms-win-core-synch-l1-2-0.dll',
  'user32.dll',
  'combase.dll',
  'shell32.dll',
  'gdi32.dll',
  'dwmapi.dll',
  'imm32.dll',
  'oleaut32.dll',
  'api-ms-win-core-winrt-error-l1-1-0.dll',
  'ADVAPI32.dll',
  'WSOCK32.dll',
  'api-ms-win-crt-math-l1-1-0.dll',
  'api-ms-win-crt-string-l1-1-0.dll',
  'api-ms-win-crt-heap-l1-1-0.dll',
  'api-ms-win-crt-stdio-l1-1-0.dll',
  'api-ms-win-crt-locale-l1-1-0.dll',
  'api-ms-win-crt-runtime-l1-1-0.dll',
  'api-ms-win-crt-utility-l1-1-0.dll',
  'api-ms-win-crt-convert-l1-1-0.dll',
  'api-ms-win-crt-filesystem-l1-1-0.dll',
  'api-ms-win-crt-time-l1-1-0.dll',
]

// DLLs of the Visual C++ Redistributable whose names start neither with MSVCP nor with VCRUNTIME (test 101).
const OTHER_REDISTRIBUTABLE_DLLS = [
  'MSVCR120.dll',
  'CONCRT140.dll',
  'VCCORLIB140.dll',
  'VCOMP140.DLL',
  'VCAMP140.dll',
  'MFC140u.dll',
  'MFCM140.dll',
]

/** What dumpbin prints when it cannot open the program `executable` (tests 101 and 124). */
function unreadableListing(executable) {
  return `LINK : fatal error LNK1181: cannot open input file '${executable}'\r\n`
}

// The job of platforms.yml that times technical design section 14's full rebuild, its step, the row of the recompute
// benchmark that the step reads, and the files that name the row and lay it out (section 2.14, tests 104 and 105).
const FULL_REBUILD = 'full-rebuild'
const FULL_REBUILD_STEP = 'Time the full rebuild of bench50'
const FULL_REBUILD_OPERATION = 'run of bench50 from an empty cache'
const RECOMPUTE_BENCHMARK = 'crates/partsmith-recompute/benches/budgets.rs'
const BUDGETS_REPORT = 'crates/partsmith-budgets/src/report.rs'

// The verdicts of the full-rebuild summary and its error, as section 2.14 and section 4 write them.
const WITHIN_BUDGET = 'Within the budget of technical design section 14.'
const OVER_BUDGET =
  'Over the budget of technical design section 14. The owner decides with an ADR that raises the budget or schedules speed work (ADR 0054).'
const NOT_MEASURED = `::error title=Full rebuild not measured::The benchmark printed no row for ${FULL_REBUILD_OPERATION}. The build or the benchmark failed, or bench50 is missing: the lines above say which (docs/spec/delivery.md section 2.14).`

// A stand-in for cargo that appends its arguments, separated by NUL, as one line to $CARGO_CALLS, prints the file
// $CARGO_STDOUT on standard output and the file $CARGO_STDERR on standard error, and exits with $CARGO_STATUS.
const CARGO_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$CARGO_CALLS"
printf '\\n' >> "$CARGO_CALLS"
cat "$CARGO_STDOUT"
cat "$CARGO_STDERR" >&2
exit "$CARGO_STATUS"
`

// The jobs of ci.yml that run the E-tests, each with the browser its steps name (the matrix's in e2e-engines) and the
// command of its test step (sections 2.4 and 2.14, tests 62, 79 and 130).
const E2E_JOBS = {
  'e2e-engines': {
    browser: '${{ matrix.browser }}',
    tests: 'xvfb-run --auto-servernum pnpm e2e --project ${{ matrix.browser }}',
  },
  e2e: { browser: 'chromium', tests: 'pnpm e2e' },
}

// The runner of both jobs, which the key of their apt cache names (section 3.2, test 130).
const E2E_RUNNER = 'ubuntu-24.04'

// The step that installs the packages of ui/ before each browser install (sections 2.4 and 2.14).
const PACKAGE_INSTALL = 'pnpm install --frozen-lockfile'

// Where the apt cache keeps the archives on the runner, and the condition of the two steps that save them: a run on
// main whose restore missed (section 3.2, test 130).
const APT_ARCHIVES = '~/apt-archives'
const AFTER_MISS_ON_MAIN =
  "steps.apt-archives.outputs.cache-hit != 'true' && github.ref == 'refs/heads/main'"

// The first line of the YAML block of section 3.2 that holds the six steps of the apt cache (test 130).
const APT_STEPS_FIRST_LINE = '- name: Read the Playwright version'

/** The command that installs `browser` with its system packages (sections 2.4 and 2.14). */
function browserInstall(browser) {
  return `pnpm exec playwright install --with-deps ${browser}`
}

/** The name of the step that installs `browser`, which GitHub's timeout error names (test 79). */
function browserInstallName(browser) {
  return `Install ${browser} and its system packages`
}

/**
 * The six steps of section 3.2 that run between the package install and the tests of a job of E2E_JOBS, for the
 * browser `browser` on the runner `runner`, each as the object that `stepEntries` gives (test 130).
 */
function aptCacheSteps(browser, runner) {
  return [
    {
      name: 'Read the Playwright version',
      id: 'playwright',
      run: [
        `version=$(node -p "require('@playwright/test/package.json').version")`,
        'echo "version=$version" >> "$GITHUB_OUTPUT"',
        '',
      ].join('\n'),
    },
    {
      name: `Restore the apt archives of ${browser}`,
      id: 'apt-archives',
      uses: pinned('actions/cache/restore'),
      with: {
        path: APT_ARCHIVES,
        key:
          `apt-archives-${runner}-${browser}-playwright-` +
          '${{ steps.playwright.outputs.version }}',
      },
    },
    {
      name: "Copy the apt archives into apt's cache",
      if: "steps.apt-archives.outputs.cache-hit == 'true'",
      run: 'sudo cp ~/apt-archives/*.deb /var/cache/apt/archives/',
    },
    {
      name: browserInstallName(browser),
      'timeout-minutes': '10',
      run: browserInstall(browser),
    },
    {
      name: "Copy the apt archives out of apt's cache",
      if: AFTER_MISS_ON_MAIN,
      run: [
        "if compgen -G '/var/cache/apt/archives/*.deb' > /dev/null; then",
        '  mkdir -p ~/apt-archives',
        '  cp /var/cache/apt/archives/*.deb ~/apt-archives/',
        'fi',
        '',
      ].join('\n'),
    },
    {
      name: `Save the apt archives of ${browser}`,
      if: AFTER_MISS_ON_MAIN,
      uses: pinned('actions/cache/save'),
      with: { path: APT_ARCHIVES, key: '${{ steps.apt-archives.outputs.cache-primary-key }}' },
    },
  ]
}

// A stand-in for gh that appends its arguments, separated by NUL, as one line to $STUB_CALLS, then prints
// $STUB_ANSWER or, when $STUB_FAILS is 1, fails as an API error does.
const GH_STUB = `#!/usr/bin/env bash
printf '%s\\0' "$@" >> "$STUB_CALLS"
printf '\\n' >> "$STUB_CALLS"
if [ "$STUB_FAILS" = 1 ]; then
  echo 'HTTP 502: Bad Gateway' >&2
  exit 1
fi
printf '%s\\n' "$STUB_ANSWER"
`

// The WebAssembly test steps of CI (section 2.4, test 66): those of kernel-wasm in order, and those of core-wasm.
const KERNEL_WASM_TESTS = [
  'cargo test -p partsmith-occt-sys --target wasm32-unknown-emscripten',
  'cargo test -p partsmith-sketch --target wasm32-unknown-emscripten',
  'cargo test -p partsmith-engine --lib --target wasm32-unknown-emscripten dispatch::',
]
const WEB_MODULE_TESTS = 'cargo test -p partsmith-wasm --test integration web_module::'
const WEB_PARITY_TESTS = 'cargo test -p partsmith-wasm --test integration web_parity:: -- --ignored'

// The engine's panic containment tests that the filter dispatch:: selects (protocol.md test 84).
const PANIC_TESTS = [
  'a_panic_is_an_internal_error_that_poisons_its_document',
  'a_poisoned_document_saves_its_recipe_from_before_the_failed_command',
]

// The variable that turns on the performance tests (ui.md 6.7, engine.md section 5), whose budgets hold only on the
// reference machine (section 1.2).
const PERFORMANCE_VARIABLE = 'PARTSMITH_PERF'

// The test steps of ci.yml's python job (section 2.4, test 73), in the order they run.
const PYTHON_SUITE = 'pytest python/tests'
const PYTHON_BUDGETS = 'pytest python/tests -m budgets -s'
const PYTHON_BUDGETS_STEP = 'Measure the Python budgets'
const PYTHON_PACKAGING = 'pytest python/tests -m packaging'

// The rows of release.yml's bundle matrix (section 3.6, test 81): the Linux packages without an AppImage (ADR 0034
// choice 1), the Windows installer and the macOS disk image.
const RELEASE_MATRIX = [
  {
    runner: 'ubuntu-24.04',
    bundles: 'deb,rpm',
    files: 'target/release/bundle/deb/*.deb target/release/bundle/rpm/*.rpm',
  },
  { runner: 'windows-2025', bundles: 'nsis', files: 'target/release/bundle/nsis/*-setup.exe' },
  { runner: 'macos-15', bundles: 'app,dmg', files: 'target/release/bundle/dmg/*.dmg' },
]

// The start of the unsigned-bundle paragraph, one line, and the files that carry it word for word (section 3.6, test 82).
const UNSIGNED_PARAGRAPH = 'These builds are not signed yet.'
const UNSIGNED_PARAGRAPH_FILES = [
  'docs/spec/delivery.md',
  '.github/workflows/AGENTS.md',
  'README.md',
]

// A reference by number to items of the release checklist: `checklist item 2`, `checklist items 4 to 6` or
// `(3.6, item 2)` (section 3.6, test 83).
const CHECKLIST_REFERENCE = /(?:checklist|\(3\.6,) items? (\d+)(?: to (\d+))?/g

// The references to the release checklist by number outside the checklist itself, in the order each file holds them,
// each with words of every item it names (section 3.6, test 83).
const CHECKLIST_REFERENCES = {
  'docs/spec/delivery.md': [
    ['platforms workflow'],
    ['platforms workflow'],
    ['platforms workflow'],
    ['source route'],
    ['platforms workflow'],
    ['platforms workflow'],
  ],
  '.github/workflows/AGENTS.md': [
    ['platforms workflow'],
    ['release dry run', 'Push the tag', 'When `draft` finishes'],
  ],
}

/** Creates a temporary directory that is removed when the test ends. */
async function temporaryDirectory(context) {
  const directory = await mkdtemp(join(tmpdir(), 'partsmith-workflows-'))
  context.after(() => rm(directory, { recursive: true, force: true }))
  return directory
}

/** The text of the workflow file `file`. */
function workflowText(file) {
  return readFile(join(workflowsDirectory, file), 'utf8')
}

/** The texts of the workflow files `files`, as `[file, text]` pairs in the order of `files`. */
async function workflowTexts(files) {
  const texts = await Promise.all(files.map(workflowText))
  return files.map((file, i) => [file, texts[i]])
}

/**
 * The jobs of a workflow's text in order, as `{ id, lines }`, read by the two-space indentation of the workflow
 * files: a job is a line `  <id>:` under `jobs:`, and its lines are the lines below it up to the next job.
 */
function workflowJobs(text) {
  const jobs = []
  let inJobs = false
  for (const line of text.split('\n')) {
    if (/^[^\s#]/.test(line)) {
      inJobs = line === 'jobs:'
      continue
    }
    const job = inJobs ? /^ {2}([\w-]+):$/.exec(line) : null
    if (job !== null) {
      jobs.push({ id: job[1], lines: [] })
    } else if (inJobs && jobs.length > 0) {
      jobs.at(-1).lines.push(line)
    }
  }
  return jobs
}

/** The job `id` of `jobs`, the jobs of the workflow file `file`; fails the test when there is no such job. */
function findJob(jobs, file, id) {
  const job = jobs.find((listed) => listed.id === id)
  assert.ok(job, `${file} has no job ${id}`)
  return job
}

/** The job `id` of the workflow file `file`; fails the test when the file has no such job. */
async function workflowJob(file, id) {
  return findJob(workflowJobs(await workflowText(file)), file, id)
}

/** The ids of the jobs of the workflow file `file`, in order. */
async function workflowJobIds(file) {
  return workflowJobs(await workflowText(file)).map((job) => job.id)
}

/**
 * The lines below the top-level key `key` of a workflow's text, such as its `on` or `env`, with their two-space
 * indentation removed and comments and blank lines left out; fails the test when the text has no such key.
 */
function topLevelLines(text, key) {
  const block = blockBelow(text.split('\n'), `${key}:`, 2)
  assert.ok(block, `the workflow has no top-level ${key}`)
  return block
}

/** The value of the key `key` of a job, the rest of its line `    key: value`; undefined when the job has none. */
function jobValue(job, key) {
  const prefix = `    ${key}:`
  const line = job.lines.find((jobLine) => jobLine.startsWith(prefix))
  return line === undefined ? undefined : line.slice(prefix.length).trim()
}

/** The `timeout-minutes` of a job as a number, or undefined when it sets none. */
function timeoutMinutes(job) {
  const value = jobValue(job, 'timeout-minutes')
  return value === undefined ? undefined : Number(value)
}

/** The entries of a value written alone or as a list `[a, b]`, in order. */
function listEntries(value) {
  const list = /^\[(.*)\]$/.exec(value)
  return list === null ? [value] : list[1].split(',').map((entry) => entry.trim())
}

/** The jobs a job needs, in order: none, one written alone, or a list written `[a, b]`. */
function jobNeeds(job) {
  const value = jobValue(job, 'needs')
  return value === undefined ? [] : listEntries(value)
}

/** The runners of a job: its matrix `runner` when it runs on `${{ matrix.runner }}`, otherwise its `runs-on`. */
function jobRunners(job) {
  const runsOn = jobValue(job, 'runs-on')
  if (runsOn !== '${{ matrix.runner }}') {
    return [runsOn]
  }
  const prefix = '  runner: '
  const line = (jobBlock(job, 'strategy') ?? []).find((strategyLine) =>
    strategyLine.startsWith(prefix),
  )
  assert.ok(line, `${job.id} runs on \${{ matrix.runner }} but its matrix has no runner`)
  return listEntries(line.slice(prefix.length).trim())
}

/**
 * The lines below the key `key` of a job, such as its `strategy`, `defaults` or `env`, with the six spaces of their
 * indentation removed and comments and blank lines left out; undefined when the job has no such key.
 */
function jobBlock(job, key) {
  return blockBelow(job.lines, `    ${key}:`, 6)
}

/**
 * The rows of a job's `strategy` matrix `include`, each as an object of its entries as written. A row starts at a line
 * `    - ` of the strategy block, and the lines indented below it belong to it.
 */
function matrixRows(job) {
  const include = blockBelow(jobBlock(job, 'strategy') ?? [], '  include:', 4) ?? []
  const rows = []
  for (const line of include) {
    if (line.startsWith('- ')) {
      rows.push([])
    }
    rows.at(-1)?.push(line.slice(2))
  }
  return rows.map(mappingEntries)
}

/**
 * The steps of a job, each as its non-blank lines with the step's indentation removed, so that its keys start
 * the lines. A step starts at a line `      - `; comments between steps belong to none.
 */
function jobSteps(job) {
  const steps = []
  for (const line of job.lines.slice(job.lines.indexOf('    steps:') + 1)) {
    if (line.startsWith('      - ')) {
      steps.push([line.slice(8)])
    } else if (line.startsWith('        ') && steps.length > 0) {
      steps.at(-1).push(line.slice(8))
    }
  }
  return steps
}

/** The value of `key` in a step: the rest of its line, or for a block scalar (`|`) the lines below it. */
function stepValue(step, key) {
  const index = step.findIndex((line) => line.startsWith(`${key}:`))
  if (index === -1) {
    return undefined
  }
  const value = step[index].slice(key.length + 1).trim()
  return value === '|' ? `${indentedLines(step, index).join('\n')}\n` : value
}

/**
 * The lines of the mapping `key` of a step, such as its `env` or `with`, with their indentation of two spaces
 * removed, so that its entries start the lines as a step's keys do; undefined when the step has no such key.
 */
function mappingLines(step, key) {
  const index = step.indexOf(`${key}:`)
  return index === -1 ? undefined : indentedLines(step, index)
}

/**
 * The entries of the mapping `key` of a step, such as its `env` or `with`, as an object of their values as written
 * (`|` for a block scalar); undefined when the step has no such key.
 */
function stepMapping(step, key) {
  const lines = mappingLines(step, key)
  return lines === undefined ? undefined : mappingEntries(lines)
}

/** The entries `key: value` of mapping lines that start at their first character, as an object of the values as written. */
function mappingEntries(lines) {
  const mapping = {}
  // The lines of a block scalar are indented further, so they match no entry and belong to the entry above them.
  for (const line of lines) {
    const entry = /^([\w-]+): ?(.*)$/.exec(line)
    if (entry !== null) {
      mapping[entry[1]] = entry[2]
    }
  }
  return mapping
}

/** A scalar as YAML reads it: the text inside matching single or double quotes, else the text itself. */
function unquoted(value) {
  const quoted = /^(["'])(.*)\1$/.exec(value)
  return quoted === null ? value : quoted[2]
}

/**
 * The lines after the line at `start` of `lines` that are indented by at least two spaces, up to the first that is
 * not, with two spaces removed.
 */
function indentedLines(lines, start) {
  const block = []
  for (const line of lines.slice(start + 1)) {
    if (!line.startsWith('  ')) {
      break
    }
    block.push(line.slice(2))
  }
  return block
}

/**
 * The lines below the line `keyLine` of `lines` that are indented by at least `indent` spaces, up to the first that is
 * not, with `indent` spaces removed and comments and blank lines left out; undefined when `lines` has no line `keyLine`.
 */
function blockBelow(lines, keyLine, indent) {
  const start = lines.indexOf(keyLine)
  if (start === -1) {
    return undefined
  }
  const prefix = ' '.repeat(indent)
  const block = []
  for (const line of lines.slice(start + 1)) {
    if (isBlankOrComment(line)) {
      continue
    }
    if (!line.startsWith(prefix)) {
      break
    }
    block.push(line.slice(indent))
  }
  return block
}

/** Whether a workflow line holds nothing but spaces or a comment. */
function isBlankOrComment(line) {
  const trimmed = line.trim()
  return trimmed === '' || trimmed.startsWith('#')
}

/** The index of the first step of `steps` for which `matches` is true; fails the test, naming `what`, when none is. */
function stepIndex(steps, what, matches) {
  const index = steps.findIndex(matches)
  assert.notEqual(index, -1, `no step ${what}`)
  return index
}

/** The index of the step of `steps` named `name`; fails the test when there is none. */
function namedStep(steps, name) {
  return stepIndex(steps, `named ${name}`, (step) => stepValue(step, 'name') === name)
}

/** The index of the step of `steps` whose `run` is `command`; fails the test when there is none. */
function commandStep(steps, command) {
  return stepIndex(steps, `that runs ${command}`, (step) => stepValue(step, 'run') === command)
}

/** The index of the step of `steps` with the `id` `id`; fails the test when there is none. */
function idStep(steps, id) {
  return stepIndex(steps, `with id ${id}`, (step) => stepValue(step, 'id') === id)
}

/** The index of the step of `steps` that uses `action` at its release of section 2.2; fails the test when there is none. */
function usesStep(steps, action) {
  const used = pinned(action)
  return stepIndex(steps, `that uses ${used}`, (step) => stepValue(step, 'uses') === used)
}

/** The index of the step of `steps` that uses `action` at any version and whose `with` sets `path` to `path`. */
function cacheStep(steps, action, path) {
  return stepIndex(
    steps,
    `that uses ${action} for ${path}`,
    (step) => usesAction(step, action) && stepMapping(step, 'with')?.path === path,
  )
}

/** The `with` of the step of `steps` that `cacheStep` finds. */
function cacheOptions(steps, action, path) {
  return stepMapping(steps[cacheStep(steps, action, path)], 'with')
}

/** The steps of the workflow `text` that use `action` at any version, in order. */
function actionSteps(text, action) {
  return workflowJobs(text).flatMap((job) =>
    jobSteps(job).filter((step) => usesAction(step, action)),
  )
}

/** Whether `step` uses `action` (its name before `@`) at any version. */
function usesAction(step, action) {
  return stepValue(step, 'uses')?.startsWith(`${action}@`) === true
}

/** The `uses` value of `action` at its release of section 2.2, such as `ilammy/msvc-dev-cmd@v1.13.0`. */
function pinned(action) {
  assert.ok(Object.hasOwn(PINNED_ACTIONS, action), `${action} is not in section 2.2`)
  return `${action}@${PINNED_ACTIONS[action]}`
}

/** The command line of a step's `run`: its value, or the one line of a block scalar without its line break. */
function stepCommand(step) {
  return stepValue(step, 'run')?.replace(/\n$/, '')
}

/** The index of the first step of `steps` at or after `start` that runs `command`; fails the test, naming `job`, when there is none. */
function commandStepFrom(steps, start, command, job) {
  const index = steps.findIndex((step, i) => i >= start && stepCommand(step) === command)
  assert.notEqual(index, -1, `${job} has no step after step ${start} that runs ${command}`)
  return index
}

/**
 * Runs the skip check of nightly.yml as GitHub runs a step without a `shell` (`bash -e`), with the gh stub first on
 * `PATH` answering `answer`, or failing when `answer` is null. Resolves to its exit status, standard output, the
 * text it wrote to `$GITHUB_OUTPUT` and the argument lists gh received.
 */
async function runSkipCheck(context, answer) {
  const properties = await workflowJob('nightly.yml', 'properties')
  const steps = jobSteps(properties)
  const step = steps[idStep(steps, 'skip')]
  const directory = await temporaryDirectory(context)
  const stubDirectory = join(directory, 'stub')
  const stubPath = join(stubDirectory, 'gh')
  const scriptPath = join(directory, 'skip.sh')
  const outputPath = join(directory, 'github-output')
  const callsPath = join(directory, 'calls')
  await mkdir(stubDirectory)
  await writeFile(stubPath, GH_STUB)
  await chmod(stubPath, 0o755)
  await writeFile(scriptPath, stepValue(step, 'run'))
  await writeFile(outputPath, '')
  await writeFile(callsPath, '')
  const result = spawnSync('bash', ['-e', scriptPath], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${stubDirectory}:${process.env.PATH}`,
      GITHUB_SHA: HEAD_COMMIT,
      GITHUB_OUTPUT: outputPath,
      STUB_CALLS: callsPath,
      STUB_ANSWER: answer ?? '',
      STUB_FAILS: answer === null ? '1' : '0',
    },
  })
  return {
    status: result.status,
    stdout: result.stdout,
    output: await readFile(outputPath, 'utf8'),
    calls: await recordedCalls(callsPath),
  }
}

/** The argument lists a stub recorded in `path`: one per line, each argument followed by NUL. */
async function recordedCalls(path) {
  return (await readFile(path, 'utf8'))
    .split('\n')
    .filter((call) => call !== '')
    .map((call) => call.split('\0').slice(0, -1))
}

test('49: every job sets the timeout-minutes of sections 2.4, 2.5, 2.11 and 2.14', async () => {
  for (const [file, text] of await workflowTexts(Object.keys(EXPECTED_TIMEOUTS))) {
    const actual = workflowJobs(text).map((job) => [job.id, timeoutMinutes(job)])
    assert.deepEqual(actual, Object.entries(EXPECTED_TIMEOUTS[file]), file)
  }
})

test('50: the schema job installs the packages first and checks the notices only when they are installed', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'schema'))
  const install = idStep(steps, 'install')
  const schemaWrite = commandStep(steps, 'cargo run -p partsmith-cli -- schema --write')
  assert.equal(stepValue(steps[install], 'run'), 'pnpm -C ui install --frozen-lockfile')
  assert.ok(install < schemaWrite, 'the install comes after schema --write')
  assert.equal(stepValue(steps.at(-1), 'run'), 'pnpm -C ui check:notices')
  assert.equal(
    stepValue(steps.at(-1), 'if'),
    "${{ !cancelled() && steps.install.outcome == 'success' }}",
  )
})

test('51: the nightly skip check skips when the newest finished run tested the head of main', async (context) => {
  const result = await runSkipCheck(context, HEAD_COMMIT)

  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=true\n')
  assert.equal(
    result.stdout,
    `::notice title=Nightly skipped::The last nightly run already tested ${HEAD_COMMIT}, the head of main, so the property tests did not run.\n`,
  )
})

/** Asserts that the skip check ran gh once, as section 2.11 says, and decided to run the tests without a message. */
function assertTests(result) {
  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=false\n')
  assert.equal(result.stdout, '')
}

test('51: the nightly skip check tests when the newest finished run tested another commit', async (context) => {
  assertTests(await runSkipCheck(context, OTHER_COMMIT))
})

test('51: the nightly skip check tests when no nightly run has finished', async (context) => {
  assertTests(await runSkipCheck(context, ''))
})

test('51: the nightly skip check tests with a warning when gh fails', async (context) => {
  const result = await runSkipCheck(context, null)

  assert.equal(result.status, 0)
  assert.deepEqual(result.calls, [SKIP_CHECK_ARGUMENTS])
  assert.equal(result.output, 'skip=false\n')
  assert.equal(
    result.stdout,
    `::warning title=Nightly skip check failed::gh run list failed, so the property tests run without checking whether an earlier night tested ${HEAD_COMMIT}.\n`,
  )
})

/** The five lines of the source stamp of section 2.13 for `commit`, each with its LF. */
function stampText(commit) {
  return [
    'Source of this deployment of the Partsmith web app',
    `Repository: ${REPOSITORY_URL}`,
    `Commit: ${commit}`,
    `Source archive: ${REPOSITORY_URL}/archive/${commit}.tar.gz`,
    `Source archive in this deployment: /${PARTSMITH_ARCHIVE}`,
  ]
    .map((line) => `${line}\n`)
    .join('')
}

/** The three lines a pull request's build adds to the source stamp (section 2.13), each with its LF. */
function pullRequestText(parents, tree) {
  return `Pull request: ${PULL_REQUEST_URL}\nParents: ${parents.join(' ')}\nTree: ${tree}\n`
}

/** The source-stamp error of section 4 for `commit`. */
function stampErrorMessage(commit) {
  return `::error title=Source stamp missing::ui/dist/${SOURCE_STAMP} does not name the commit ${commit}, so nothing was deployed. The web job writes it (docs/spec/delivery.md section 2.13).`
}

/**
 * Runs a step's `run` script with `bash -e` in `directory` and `environment`, as GitHub runs a step without a `shell`,
 * or with `bash -e -o pipefail` when `pipefail` is set, as GitHub runs a step whose `shell` is `bash`. With
 * `ignorePipe`, the shell that starts it ignores SIGPIPE (`trap '' PIPE`), which the step's commands inherit.
 */
async function runStep(
  context,
  step,
  directory,
  environment,
  { ignorePipe = false, pipefail = false } = {},
) {
  const scriptPath = join(await temporaryDirectory(context), 'step.sh')
  await writeFile(scriptPath, stepValue(step, 'run'))
  const options = pipefail ? ['-e', '-o', 'pipefail'] : ['-e']
  const command = ignorePipe
    ? ['-c', `trap '' PIPE; bash ${options.join(' ')} "$1"`, 'bash', scriptPath]
    : [...options, scriptPath]
  return spawnSync('bash', command, {
    cwd: directory,
    env: environment,
    encoding: 'utf8',
  })
}

/** The step `Write the source stamp` of the web job of ci.yml. */
async function stampWriter() {
  const steps = jobSteps(await workflowJob('ci.yml', 'web'))
  return steps[namedStep(steps, 'Write the source stamp')]
}

test('52: the web job writes the source stamp after the build and before the distribution check', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'web'))
  const build = commandStep(steps, 'pnpm build')
  const writer = namedStep(steps, 'Write the source stamp')
  const check = commandStep(steps, 'pnpm check:dist')

  assert.ok(
    build < writer && writer < check,
    'Write the source stamp is not between pnpm build and pnpm check:dist',
  )
  assert.equal(stepValue(steps[writer], 'shell'), undefined)
  assert.deepEqual(stepMapping(steps[writer], 'env'), {
    REPOSITORY_URL: '${{ github.server_url }}/${{ github.repository }}',
    PULL_REQUEST_URL: '${{ github.event.pull_request.html_url }}',
  })
})

test('52: the source stamp of a push names the repository, the commit and its source archive', async (context) => {
  const build = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(build)
  await mkdir(join(build, 'dist'))

  const result = await runStep(context, await stampWriter(), build, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL: '',
    GITHUB_SHA: HEAD_COMMIT,
  })

  assert.equal(result.status, 0, result.stderr)
  assert.equal(await readFile(join(build, 'dist', SOURCE_STAMP), 'utf8'), stampText(HEAD_COMMIT))
})

/**
 * Makes a repository in `directory` whose tip is a merge, made with --no-ff, of a head commit on another branch into a
 * base commit, with a message whose second paragraph holds a decoy parent line and a decoy tree line. Returns the
 * repository's path and the merge commit, its two parents and its tree.
 */
async function mergedRepository(environment, directory) {
  const repository = join(directory, 'repository')
  await mkdir(repository)
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFile(join(repository, 'base.txt'), 'base\n')
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Base')
  const base = git(environment, repository, 'rev-parse', 'HEAD')
  git(environment, repository, 'switch', '--create', 'head')
  await writeFile(join(repository, 'head.txt'), 'head\n')
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Head')
  const head = git(environment, repository, 'rev-parse', 'HEAD')
  git(environment, repository, 'switch', 'main')
  const decoy = `parent ${'c'.repeat(40)}\ntree ${'d'.repeat(40)}`
  git(
    environment,
    repository,
    'merge',
    '--no-ff',
    '--message',
    `Merge head into main\n\n${decoy}`,
    'head',
  )
  const merge = git(environment, repository, 'rev-parse', 'HEAD')
  assert.ok(git(environment, repository, 'cat-file', 'commit', merge).endsWith(`\n\n${decoy}`))
  return {
    repository,
    merge,
    parents: [base, head],
    tree: git(environment, repository, 'rev-parse', `${merge}^{tree}`),
  }
}

/** A shallow clone (depth 1) of `repository` in `directory`, holding an empty `dist/`, as the web job checks out. */
async function shallowClone(environment, repository, directory) {
  const clone = join(directory, 'clone')
  git(
    environment,
    directory,
    'clone',
    '--quiet',
    '--depth',
    '1',
    pathToFileURL(repository).href,
    clone,
  )
  await mkdir(join(clone, 'dist'))
  return clone
}

test('52: the source stamp of a pull request also names it, the two parents of the merge and its tree', async (context) => {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const { repository, merge, parents, tree } = await mergedRepository(environment, directory)
  const clone = await shallowClone(environment, repository, directory)

  const result = await runStep(context, await stampWriter(), clone, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL,
    GITHUB_SHA: merge,
  })

  assert.equal(result.status, 0, result.stderr)
  assert.equal(
    await readFile(join(clone, 'dist', SOURCE_STAMP), 'utf8'),
    stampText(merge) + pullRequestText(parents, tree),
  )
})

test('52: the source stamp of a pull request fails when the checkout does not hold the commit', async (context) => {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const { repository, parents } = await mergedRepository(environment, directory)
  const clone = await shallowClone(environment, repository, directory)

  // The base is a parent of the clone's only commit, so the shallow clone does not hold it.
  const result = await runStep(context, await stampWriter(), clone, {
    ...environment,
    REPOSITORY_URL,
    PULL_REQUEST_URL,
    GITHUB_SHA: parents[0],
  })

  assert.notEqual(result.status, 0)
})

/** The steps of the deploy job of ci.yml and the index of its step `Check the source stamp`. */
async function deploySteps() {
  const steps = jobSteps(await workflowJob('ci.yml', 'deploy'))
  return { steps, check: namedStep(steps, 'Check the source stamp') }
}

test('53: the deploy job checks the source stamp after downloading web-dist and before Wrangler', async () => {
  const { steps, check } = await deploySteps()
  const download = stepIndex(
    steps,
    'that downloads web-dist',
    (step) =>
      usesAction(step, 'actions/download-artifact') &&
      stepMapping(step, 'with')?.name === 'web-dist',
  )
  const pages = idStep(steps, 'pages')

  assert.equal(stepValue(steps[check], 'if'), "steps.secrets.outputs.available == 'true'")
  assert.ok(
    download < check && check < pages,
    'Check the source stamp is not between the download and Wrangler',
  )
})

/**
 * Runs the deploy job's stamp check in a new folder with `GITHUB_SHA` forty `a` digits, after writing `stamp` to
 * `ui/dist/SOURCE.txt` (no file when it is null, and no `ui/dist` when it is undefined).
 */
async function runStampCheck(context, stamp) {
  const { steps, check } = await deploySteps()
  const workspace = await temporaryDirectory(context)
  if (stamp !== undefined) {
    await mkdir(join(workspace, 'ui/dist'), { recursive: true })
  }
  if (typeof stamp === 'string') {
    await writeFile(join(workspace, 'ui/dist', SOURCE_STAMP), stamp)
  }
  return runStep(context, steps[check], workspace, { ...process.env, GITHUB_SHA: HEAD_COMMIT })
}

/** Asserts that the stamp check passes and prints nothing with `stamp` in `ui/dist/SOURCE.txt`. */
async function assertStampAccepted(context, stamp) {
  const result = await runStampCheck(context, stamp)

  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout, '')
  assert.equal(result.stderr, '')
}

/** Asserts that the stamp check fails with exactly the source-stamp error, as `runStampCheck` takes `stamp`. */
async function assertStampRejected(context, stamp) {
  const result = await runStampCheck(context, stamp)

  assert.equal(result.status, 1)
  assert.equal(result.stdout, `${stampErrorMessage(HEAD_COMMIT)}\n`)
  assert.equal(result.stderr, '')
}

test('53: the stamp check passes silently when the stamp of a push names the commit', (context) =>
  assertStampAccepted(context, stampText(HEAD_COMMIT)))

test('53: the stamp check passes silently when the stamp of a pull request names the commit', (context) =>
  assertStampAccepted(
    context,
    stampText(HEAD_COMMIT) + pullRequestText([OTHER_COMMIT, 'c'.repeat(40)], 'd'.repeat(40)),
  ))

test('53: the stamp check fails with the source-stamp error when the stamp is missing', (context) =>
  assertStampRejected(context, null))

test('53: the stamp check fails with the source-stamp error when ui/dist is missing', (context) =>
  assertStampRejected(context, undefined))

test('53: the stamp check fails with the source-stamp error when the stamp names another commit', (context) =>
  assertStampRejected(context, stampText(OTHER_COMMIT)))

test('53: the stamp check fails with the source-stamp error when the commit line runs on', (context) =>
  assertStampRejected(
    context,
    stampText(HEAD_COMMIT).replace(`Commit: ${HEAD_COMMIT}\n`, `Commit: ${HEAD_COMMIT}0\n`),
  ))

/** Whether `step` downloads the artifact `name` into `path` with a `with` of exactly those two entries. */
function downloadsArtifact(step, name, path) {
  return (
    stepValue(step, 'uses') === pinned('actions/download-artifact') &&
    isDeepStrictEqual(stepMapping(step, 'with'), { name, path })
  )
}

test('54: the draft job downloads the source artifact after the bundles and before the notices and SHA256SUMS', async () => {
  const steps = jobSteps(await workflowJob('release.yml', 'draft'))
  const bundles = stepIndex(
    steps,
    'that downloads the bundle-* artifacts',
    (step) =>
      usesAction(step, 'actions/download-artifact') &&
      stepMapping(step, 'with')?.pattern === 'bundle-*',
  )
  const sources = stepIndex(steps, 'that downloads sources-desktop into release', (step) =>
    downloadsArtifact(step, 'sources-desktop', 'release'),
  )
  const notices = commandStep(steps, 'cp THIRD-PARTY-NOTICES.txt release/')
  const sums = namedStep(steps, 'Write SHA256SUMS')

  assert.ok(
    bundles < sources && sources < notices && notices < sums,
    'the download of sources-desktop is not between the bundles and the notices, before SHA256SUMS',
  )
  assert.match(stepValue(steps.at(-1), 'run'), /^gh release create .* release\/\*$/m)
})

test('54: the draft job makes none of the archives again', async () => {
  const steps = jobSteps(await workflowJob('release.yml', 'draft'))

  assert.ok(!steps.some((step) => usesAction(step, 'actions/setup-node')), 'draft sets up Node')
  for (const word of ['rustup', ARCHIVE_SCRIPT, PARTSMITH_SCRIPT, DEPENDENCY_SCRIPT]) {
    assert.ok(
      !steps.some((step) => stepValue(step, 'run')?.includes(word)),
      `a step of draft runs ${word}`,
    )
  }
})

test('54: both archive scripts are regular files their owner can run', async () => {
  const scripts = [ARCHIVE_SCRIPT, PARTSMITH_SCRIPT]
  const statuses = await Promise.all(scripts.map((script) => lstat(join(repositoryRoot, script))))

  for (const [i, status] of statuses.entries()) {
    assert.ok(status.isFile(), `${scripts[i]} is not a regular file`)
    assert.notEqual(status.mode & 0o100, 0, `${scripts[i]} is not executable by its owner`)
  }
})

test('57: the notices statements name the stamp, both source archives and their scripts', async () => {
  const { version } = await readOcctPins()
  const archiveName = sourceArchiveName(version)
  const web = await repositoryText('ui/public/THIRD-PARTY-NOTICES.txt')
  const desktop = await repositoryText('THIRD-PARTY-NOTICES.txt')

  // The web statement names each archive at its path in the deployment, where tests 87 and 120 see deploy check it.
  for (const name of [
    `/${SOURCE_STAMP}`,
    `/${PARTSMITH_ARCHIVE}`,
    PARTSMITH_SCRIPT,
    `/${archiveName}`,
    ARCHIVE_SCRIPT,
  ]) {
    assert.ok(web.includes(name), `ui/public/THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
  for (const name of [PARTSMITH_ARCHIVE, PARTSMITH_SCRIPT, archiveName, ARCHIVE_SCRIPT]) {
    assert.ok(desktop.includes(name), `THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
})

test('59: every artifact the workflows upload is kept 7 days, and each source artifact 8', async () => {
  for (const [file, text] of await workflowTexts(Object.keys(EXPECTED_UPLOADS))) {
    const uploads = actionSteps(text, 'actions/upload-artifact').map((step) =>
      stepMapping(step, 'with'),
    )

    assert.deepEqual(
      uploads.map((upload) => [upload?.name, upload?.['retention-days']]),
      EXPECTED_UPLOADS[file].map((name) => [
        name,
        name.startsWith('sources-') ? SOURCE_RETENTION_DAYS : ARTIFACT_RETENTION_DAYS,
      ]),
      file,
    )
  }
})

test('59: each source artifact is the folder sources/, stored as it is, and never missing', async () => {
  for (const [file, text] of await workflowTexts(Object.keys(SOURCE_ARTIFACTS))) {
    const upload = actionSteps(text, 'actions/upload-artifact').find(
      (step) => stepMapping(step, 'with')?.name === SOURCE_ARTIFACTS[file].artifact,
    )
    assert.ok(upload, `${file} uploads no ${SOURCE_ARTIFACTS[file].artifact}`)

    assert.deepEqual(
      stepMapping(upload, 'with'),
      {
        name: SOURCE_ARTIFACTS[file].artifact,
        path: `${SOURCES_FOLDER}/`,
        'retention-days': SOURCE_RETENTION_DAYS,
        'if-no-files-found': 'error',
        'compression-level': '0',
      },
      file,
    )
  }
})

/** Whether `step` downloads the artifact `core-wasm` into `ui/public`. */
function downloadsCoreIntoPublic(step) {
  const options = stepMapping(step, 'with')
  return (
    usesAction(step, 'actions/download-artifact') &&
    options?.name === 'core-wasm' &&
    options.path === 'ui/public'
  )
}

test('59: every core-wasm artifact holds the web notices next to the core, and the jobs that need it download it into ui/public', async () => {
  for (const [file, text] of await workflowTexts(Object.keys(CORE_DOWNLOADS))) {
    const upload = actionSteps(text, 'actions/upload-artifact').find(
      (step) => stepMapping(step, 'with')?.name === 'core-wasm',
    )
    assert.ok(upload, `${file} uploads no core-wasm artifact`)
    const jobs = workflowJobs(text)

    assert.equal(
      stepValue(mappingLines(upload, 'with'), 'path'),
      'ui/public/core/\nui/public/THIRD-PARTY-NOTICES.txt\n',
      file,
    )
    // The jobs alone would not catch an artifact without the notices: they take them from their checkout.
    for (const id of CORE_DOWNLOADS[file]) {
      stepIndex(
        jobSteps(findJob(jobs, file, id)),
        `in ${id} of ${file} that downloads core-wasm into ui/public`,
        downloadsCoreIntoPublic,
      )
    }
  }
})

/** The OCCT native cache key of release.yml's bundle job, which the platforms jobs share (sections 2.14 and 3.2). */
async function bundleOcctKey() {
  const steps = jobSteps(await workflowJob('release.yml', 'bundle'))
  return cacheOptions(steps, 'actions/cache', '.occt/native').key
}

/** The step of release.yml's bundle job named `name`. */
async function bundleStep(name) {
  const steps = jobSteps(await workflowJob('release.yml', 'bundle'))
  return steps[namedStep(steps, name)]
}

test('60: the platforms workflow runs only on a schedule and on demand', async () => {
  const on = topLevelLines(await workflowText('platforms.yml'), 'on')
  const keys = on.filter((line) => /^\S/.test(line)).map((line) => line.replace(/:.*$/, ''))
  const schedules = on.filter((line) => line.startsWith('  - '))

  assert.deepEqual(keys, ['schedule', 'workflow_dispatch'])
  assert.equal(schedules.length, 1, 'schedule has exactly one entry')
  const cron = /^ {2}- cron: (.*)$/.exec(schedules[0])
  assert.ok(cron, `the schedule entry ${schedules[0]} is not a cron`)
  assert.equal(unquoted(cron[1]), '41 5 * * 1')
})

test('60: the platforms workflow reads the repository only, queues its runs and shares the environment of CI', async () => {
  const text = await workflowText('platforms.yml')

  assert.deepEqual(topLevelLines(text, 'permissions'), ['contents: read'])
  assert.deepEqual(mappingEntries(topLevelLines(text, 'concurrency')), {
    group: 'platforms-${{ github.ref }}',
    'cancel-in-progress': 'false',
  })
  assert.deepEqual(
    mappingEntries(topLevelLines(text, 'env')),
    mappingEntries(topLevelLines(await workflowText('ci.yml'), 'env')),
  )
})

test('60: the platforms workflow holds the copy of the macOS OCCT row and full-rebuild, which needs it', async () => {
  const jobs = workflowJobs(await workflowText('platforms.yml'))

  assert.deepEqual(
    jobs.map((job) => [job.id, jobNeeds(job)]),
    [
      ['occt-platforms', []],
      ['full-rebuild', ['occt-platforms']],
    ],
  )
})

test('60: no step of the platforms workflow is allowed to fail', async () => {
  for (const job of workflowJobs(await workflowText('platforms.yml'))) {
    for (const step of jobSteps(job)) {
      assert.equal(
        stepValue(step, 'continue-on-error'),
        undefined,
        `a step of ${job.id} sets continue-on-error`,
      )
    }
  }
})

test('60: every promoted job gates the deploy, and no job of the platforms workflow does', async () => {
  const deploy = await workflowJob('ci.yml', 'deploy')

  assert.deepEqual(jobNeeds(deploy), DEPLOY_NEEDS)
})

test('61: the Windows and macOS jobs run in Git Bash on both runners, and one runner never cancels the other', async () => {
  const jobs = await Promise.all(PLATFORM_JOBS.map((id) => workflowJob('ci.yml', id)))
  for (const job of jobs) {
    const { id } = job

    assert.equal(jobValue(job, 'runs-on'), '${{ matrix.runner }}', id)
    assert.deepEqual(
      jobBlock(job, 'strategy'),
      ['fail-fast: false', 'matrix:', '  runner: [windows-2025, macos-15]'],
      id,
    )
    assert.deepEqual(jobBlock(job, 'defaults'), ['run:', '  shell: bash'], id)
  }
})

test('61: the Rust tests run after a failed OCCT build on the other runner', async () => {
  const job = await workflowJob('ci.yml', 'rust-platforms')

  assert.equal(jobValue(job, 'if'), '${{ !cancelled() }}')
})

test("61: both platform jobs use the OCCT cache key of the release's bundle job", async () => {
  const key = await bundleOcctKey()
  const occt = jobSteps(await workflowJob('ci.yml', 'occt-platforms'))
  const rust = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const cache = occt[cacheStep(occt, 'actions/cache', '.occt/native')]
  const restore = cacheOptions(rust, 'actions/cache/restore', '.occt/native')

  assert.equal(stepValue(cache, 'uses'), pinned('actions/cache'))
  assert.equal(stepMapping(cache, 'with').key, key)
  assert.equal(restore.key, key)
  assert.equal(restore['fail-on-cache-miss'], 'true')
})

test('61: occt-platforms sets up the compiler and builds OCCT only on a cache miss', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'occt-platforms'))
  const miss = "steps.occt.outputs.cache-hit != 'true'"
  const cache = cacheStep(steps, 'actions/cache', '.occt/native')
  const msvc = usesStep(steps, 'ilammy/msvc-dev-cmd')
  const ninja = commandStep(
    steps,
    stepValue(await bundleStep('Install Ninja and set the deployment target'), 'run'),
  )
  const build = commandStep(steps, 'scripts/build-occt.sh native "$OCCT_ROOT"')

  assert.equal(stepValue(steps[cache], 'id'), 'occt')
  assert.deepEqual(stepMapping(steps[build], 'env'), { OCCT_JOBS: '4' })
  assert.ok(stepValue(steps[build], 'if')?.includes(miss), 'the build runs only on a cache miss')
  assert.ok(cache < msvc && cache < ninja, 'the compiler setup comes before the cache step')
  assert.ok(msvc < build && ninja < build, 'the compiler setup comes after the build')
  for (const [index, os] of [
    [msvc, 'Windows'],
    [ninja, 'macOS'],
  ]) {
    const condition = stepValue(steps[index], 'if') ?? ''
    assert.ok(condition.includes(`runner.os == '${os}'`), `the ${os} setup runs on ${os} only`)
    assert.ok(condition.includes(miss), `the ${os} setup runs only on a cache miss`)
  }
})

test('61: occt-platforms prints the free disk space after the build, also when it fails', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'occt-platforms'))
  const last = steps.at(-1)

  assert.equal(stepValue(last, 'name'), 'Disk space after the build')
  assert.equal(stepValue(last, 'run'), 'df -h .')
  assert.equal(stepValue(last, 'if'), 'always()')
})

test('61: rust-platforms links with MSVC on Windows as the release does', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const release = await bundleStep('Use the MSVC linker for Rust')
  const msvc = usesStep(steps, 'ilammy/msvc-dev-cmd')
  const linker = namedStep(steps, 'Use the MSVC linker for Rust')

  assert.equal(stepValue(steps[msvc], 'if'), "runner.os == 'Windows'")
  assert.ok(msvc < linker, 'the developer environment is set up before the linker is looked up')
  assert.equal(stepValue(steps[linker], 'run'), stepValue(release, 'run'))
  assert.equal(stepValue(steps[linker], 'if'), stepValue(release, 'if'))
})

test('61: rust-platforms builds for the macOS deployment target of OCCT and the app', async (context) => {
  const steps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const step = steps[namedStep(steps, 'Set the deployment target')]
  const directory = await temporaryDirectory(context)
  const environmentFile = join(directory, 'github-env')
  await writeFile(environmentFile, '')

  const result = await runStep(context, step, directory, {
    ...process.env,
    GITHUB_ENV: environmentFile,
  })

  assert.equal(stepValue(step, 'if'), "runner.os == 'macOS'")
  assert.equal(result.status, 0, result.stderr)
  assert.equal(await readFile(environmentFile, 'utf8'), 'MACOSX_DEPLOYMENT_TARGET=13.0\n')
})

test('61: rust-platforms saves its Rust cache only on main, also when its tests fail', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const cache = usesStep(steps, 'Swatinem/rust-cache')

  assert.deepEqual(stepMapping(steps[cache], 'with'), {
    'save-if': "${{ github.ref == 'refs/heads/main' }}",
    'cache-on-failure': 'true',
  })
})

test('61: rust-platforms runs the crate tests, the examples runner and clippy, then prints the free disk space', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const crates = idStep(steps, 'crates')
  const [examples, clippy] = steps.slice(crates + 1)
  // The last step, whatever steps run between clippy and it, such as the import check of test 100.
  const disk = steps.at(-1)

  assert.equal(stepValue(steps[crates], 'run'), CRATE_TESTS)
  assert.ok(examples, 'no step after the crate tests')
  assert.ok(examples.includes('run: |'), 'the examples runner is a block scalar')
  assert.equal(stepValue(examples, 'run'), EXAMPLES_RUNNER)
  assert.equal(stepValue(examples, 'if'), AFTER_CRATE_TESTS)
  assert.ok(clippy, 'no step after the examples runner')
  assert.equal(stepValue(clippy, 'run'), PLATFORM_CLIPPY)
  assert.equal(stepValue(clippy, 'if'), AFTER_CRATE_TESTS)
  assert.equal(stepValue(disk, 'name'), 'Disk space after the tests')
  assert.equal(stepValue(disk, 'run'), 'df -h .')
  assert.equal(stepValue(disk, 'if'), 'always()')
})

test('61: a hung crate test ends its own step after 20 minutes, so the examples runner and clippy still run', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const crates = idStep(steps, 'crates')

  assert.equal(stepValue(steps[crates], 'timeout-minutes'), '20')
})

test("62: e2e-engines runs the engine subset in Firefox and in WebKit against CI's core-wasm", async () => {
  const job = await workflowJob('ci.yml', 'e2e-engines')
  const steps = jobSteps(job)
  const { browser, tests } = E2E_JOBS['e2e-engines']
  const install = commandStep(steps, PACKAGE_INSTALL)
  const browsers = commandStep(steps, browserInstall(browser))
  const run = commandStep(steps, tests)

  assert.deepEqual(jobNeeds(job), ['core-wasm'])
  assert.equal(jobValue(job, 'runs-on'), 'ubuntu-24.04')
  assert.deepEqual(jobBlock(job, 'strategy'), [
    'fail-fast: false',
    'matrix:',
    '  browser: [firefox, webkit]',
  ])
  assert.deepEqual(jobBlock(job, 'defaults'), ['run:', '  working-directory: ui'])
  assert.ok(
    install < browsers && browsers < run,
    'the install, the browser and the run are out of order',
  )
  assert.equal(stepMapping(steps[run], 'env'), undefined)
  assert.equal(stepValue(steps.at(-1), 'uses'), pinned('actions/upload-artifact'))
  assert.equal(stepValue(steps.at(-1), 'if'), 'failure()')
})

test("62: CI's e2e installs Chromium alone and runs the default project", async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'e2e'))
  const browser = commandStep(steps, 'pnpm exec playwright install --with-deps chromium')
  const run = commandStep(steps, 'pnpm e2e')

  assert.ok(browser < run, 'Chromium is installed after the run')
})

/** Every `uses:` value of a workflow's text, in order, without a comment after it. */
function usedActions(text) {
  return text
    .split('\n')
    .map((line) =>
      /^\s*(?:- )?uses:(.*)$/
        .exec(line)?.[1]
        .replace(/\s#.*$/, '')
        .trim(),
    )
    .filter((used) => used !== undefined)
}

test('63: every action is pinned to the exact release of section 2.2', async () => {
  for (const [file, text] of await workflowTexts(WORKFLOW_FILES)) {
    for (const used of usedActions(text)) {
      const at = used.lastIndexOf('@')
      const action = used.slice(0, at)

      assert.match(used, /^[\w./-]+@[\w.-]+$/, `${file}: ${used} is not <action>@<version>`)
      assert.ok(Object.hasOwn(PINNED_ACTIONS, action), `${file}: ${action} is not in section 2.2`)
      assert.equal(used.slice(at + 1), PINNED_ACTIONS[action], `${file}: ${action}`)
    }
  }
})

test('63: every action of section 2.2 is used by a workflow', async () => {
  const texts = await Promise.all(WORKFLOW_FILES.map(workflowText))
  const used = new Set(
    texts.flatMap(usedActions).map((action) => action.slice(0, action.lastIndexOf('@'))),
  )

  for (const action of Object.keys(PINNED_ACTIONS)) {
    assert.ok(used.has(action), `no workflow uses ${action}`)
  }
})

/** The text of the repository file at `path`, a path from the repository root. */
function repositoryText(path) {
  return readFile(join(repositoryRoot, path), 'utf8')
}

test('66: kernel-wasm runs the kernel, sketch and engine panic tests on WebAssembly after its Rust cache', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'kernel-wasm'))
  let previous = usesStep(steps, 'Swatinem/rust-cache')

  for (const command of KERNEL_WASM_TESTS) {
    previous = commandStepFrom(steps, previous + 1, command, 'kernel-wasm')
  }
})

test('66: core-wasm runs the round trips with the module required, then the parity test, between the build and the upload', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'core-wasm'))
  const build = commandStep(steps, 'scripts/build-wasm.sh')
  const roundTrips = commandStepFrom(steps, build + 1, WEB_MODULE_TESTS, 'core-wasm')
  const parity = commandStepFrom(steps, roundTrips + 1, WEB_PARITY_TESTS, 'core-wasm')
  const upload = usesStep(steps, 'actions/upload-artifact')

  assert.equal(
    unquoted(stepMapping(steps[roundTrips], 'env')?.PARTSMITH_REQUIRE_WEB_MODULE ?? ''),
    '1',
  )
  assert.ok(parity < upload, 'the parity test runs after the upload of core-wasm')
})

test('66: the filters web_module:: and web_parity:: select modules of the integration binary of partsmith-wasm', async () => {
  const integration = await repositoryText('crates/partsmith-wasm/tests/integration.rs')

  for (const module of ['web_module', 'web_parity']) {
    assert.match(
      integration,
      new RegExp(`^mod ${module};$`, 'm'),
      `integration.rs declares mod ${module};`,
    )
  }
})

test('66: the filter web_parity:: selects an ignored test, which -- --ignored runs', async () => {
  const lines = (await repositoryText('crates/partsmith-wasm/tests/web_parity.rs'))
    .split('\n')
    .map((line) => line.trim())

  assert.ok(
    lines.some(
      (line, i) =>
        line === '#[test]' &&
        [lines[i - 1], lines[i + 1]].some((next) => next?.startsWith('#[ignore')),
    ),
    'web_parity.rs has no #[test] with an #[ignore attribute next to it',
  )
})

test('66: the filter dispatch:: selects the two panic tests that protocol.md test 84 names', async () => {
  const dispatch = await repositoryText('crates/partsmith-engine/src/dispatch.rs')

  for (const name of PANIC_TESTS) {
    assert.match(dispatch, new RegExp(`^\\s*fn ${name}\\(`, 'm'), `dispatch.rs defines fn ${name}`)
  }
})

test('66: a run written as a block scalar is read as its one command line', () => {
  assert.equal(stepCommand(['run: |', `  ${WEB_PARITY_TESTS}`]), WEB_PARITY_TESTS)
  assert.equal(stepCommand(['run: scripts/build-wasm.sh']), 'scripts/build-wasm.sh')
  assert.equal(stepCommand(['run: |', '  cargo build', '  cargo test']), 'cargo build\ncargo test')
})

test('67: no workflow file names PARTSMITH_PERF, so no job runs the performance tests', async () => {
  const files = (await readdir(workflowsDirectory))
    .filter((name) => name.endsWith('.yml'))
    .toSorted()
  for (const file of WORKFLOW_FILES) {
    assert.ok(files.includes(file), `.github/workflows holds ${file}`)
  }

  for (const [file, text] of await workflowTexts(files)) {
    assert.ok(!text.includes(PERFORMANCE_VARIABLE), `${file} names ${PERFORMANCE_VARIABLE}`)
  }
})

/** The indexes of the python job's suite, budgets and packaging steps, each found after the one before it. */
function pythonTestSteps(steps) {
  const suite = commandStepFrom(steps, 0, PYTHON_SUITE, 'python')
  const budgets = commandStepFrom(steps, suite + 1, PYTHON_BUDGETS, 'python')
  const packaging = commandStepFrom(steps, budgets + 1, PYTHON_PACKAGING, 'python')
  return [suite, budgets, packaging]
}

test('73: the python job measures the budgets after the suite and before the wheel', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'python'))
  const [, budgets] = pythonTestSteps(steps)

  assert.equal(stepValue(steps[budgets], 'name'), PYTHON_BUDGETS_STEP)
})

test('73: no test step of the python job may fail without failing the job', async () => {
  const steps = jobSteps(await workflowJob('ci.yml', 'python'))

  for (const index of pythonTestSteps(steps)) {
    assert.equal(
      stepValue(steps[index], 'continue-on-error'),
      undefined,
      `the python step that runs ${stepCommand(steps[index])} sets continue-on-error`,
    )
  }
})

test('79: a slow mirror ends each browser install after 10 minutes, not the job', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))

  for (const [id, { browser }] of Object.entries(E2E_JOBS)) {
    const job = findJob(jobs, 'ci.yml', id)
    const steps = jobSteps(job)
    const install = commandStep(steps, browserInstall(browser))

    assert.equal(stepValue(steps[install], 'name'), browserInstallName(browser), id)
    assert.equal(stepValue(steps[install], 'timeout-minutes'), '10', id)
    assert.ok(10 < timeoutMinutes(job), `${id}: the browser install may take the whole job's time`)
  }
})

test('79: after a failed install, each report upload only warns that it found no files', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))

  for (const id of Object.keys(E2E_JOBS)) {
    const upload = jobSteps(findJob(jobs, 'ci.yml', id)).at(-1)

    assert.equal(stepValue(upload, 'uses'), pinned('actions/upload-artifact'), id)
    assert.equal(stepValue(upload, 'if'), 'failure()', id)
    assert.equal(stepMapping(upload, 'with')['if-no-files-found'], undefined, id)
  }
})

test('81: the release matrix builds the bundles of section 3.6 and no AppImage', async () => {
  const rows = matrixRows(await workflowJob('release.yml', 'bundle'))

  assert.deepEqual(rows, RELEASE_MATRIX)
})

/**
 * The items of the first numbered list below the first line of the repository file `path` that starts with `start`,
 * each without its number; fails the test when there is no such line or list, or when the list is not numbered from
 * 1 with no gap.
 */
async function checklistItems(path, start) {
  const lines = (await repositoryText(path)).split('\n')
  const first = lines.findIndex((line) => line.startsWith(start))
  assert.notEqual(first, -1, `${path} has no line that starts with ${start}`)
  const items = []
  for (const line of lines.slice(first + 1)) {
    const item = /^(\d+)\. (.*)$/.exec(line)
    if (item === null) {
      if (items.length > 0) {
        break
      }
      continue
    }
    assert.equal(
      Number(item[1]),
      items.length + 1,
      `${path} numbers its checklist item ${items.length + 1} as ${item[1]}`,
    )
    items.push(item[2])
  }
  assert.ok(items.length > 0, `${path} has no numbered list below ${start}`)
  return items
}

test('82: the release checklist of the workflows AGENTS.md is the one of section 3.6, word for word', async () => {
  assert.deepEqual(
    await checklistItems('.github/workflows/AGENTS.md', '## Release checklist'),
    await checklistItems('docs/spec/delivery.md', '**Release checklist**'),
  )
})

test('82: the unsigned-bundle paragraph is the same one line in section 3.6, the workflows AGENTS.md and README.md', async () => {
  const texts = await Promise.all(UNSIGNED_PARAGRAPH_FILES.map(repositoryText))
  const paragraphs = UNSIGNED_PARAGRAPH_FILES.map((path, i) => {
    const lines = texts[i].split('\n').filter((line) => line.startsWith(UNSIGNED_PARAGRAPH))
    assert.equal(
      lines.length,
      1,
      `${path} holds ${lines.length} lines that start with ${UNSIGNED_PARAGRAPH}`,
    )
    return lines[0]
  })

  for (const [i, path] of UNSIGNED_PARAGRAPH_FILES.entries()) {
    assert.equal(
      paragraphs[i],
      paragraphs[0],
      `${path} words the paragraph unlike ${UNSIGNED_PARAGRAPH_FILES[0]}`,
    )
  }
})

test('83: each release checklist item named by number outside the checklist is the item meant', async () => {
  const items = await checklistItems('docs/spec/delivery.md', '**Release checklist**')
  const files = Object.entries(CHECKLIST_REFERENCES)
  const texts = await Promise.all(files.map(([path]) => repositoryText(path)))

  for (const [i, [path, expected]] of files.entries()) {
    const references = [...texts[i].matchAll(CHECKLIST_REFERENCE)]
    assert.equal(
      references.length,
      expected.length,
      `${path} names checklist items by number in: ${references.map((reference) => reference[0]).join(', ')}`,
    )
    for (const [position, reference] of references.entries()) {
      const first = Number(reference[1])
      const last = Number(reference[2] ?? reference[1])
      assert.equal(
        last - first + 1,
        expected[position].length,
        `${path}: ${reference[0]} names another number of items`,
      )
      for (const [offset, words] of expected[position].entries()) {
        assert.ok(
          items[first + offset - 1]?.includes(words),
          `${path}: ${reference[0]} names item ${first + offset}, which does not hold "${words}"`,
        )
      }
    }
  }
})

/** The steps of the job `id` of ci.yml. */
async function ciJobSteps(id) {
  return jobSteps(await workflowJob('ci.yml', id))
}

/** The sources job of each workflow file of SOURCE_ARTIFACTS, in that order, as `{ file, job, steps }`. */
async function sourcesJobs() {
  return Promise.all(
    Object.keys(SOURCE_ARTIFACTS).map(async (file) => {
      const job = await workflowJob(file, SOURCES_JOB)
      return { file, job, steps: jobSteps(job) }
    }),
  )
}

// The workflow whose sources job makes the OCCT archive in every run, with no cache (section 2.13, Made in the sources
// jobs).
const UNCACHED_ARCHIVE_FILE = 'release.yml'

/** The sources jobs of `sourcesJobs` that restore the OCCT archive's cache: ci.yml's. */
async function cachingSourcesJobs() {
  return (await sourcesJobs()).filter(({ file }) => file !== UNCACHED_ARCHIVE_FILE)
}

/** The index of the step of a sources job's `steps` that makes the OCCT archive, after the cache step if there is one. */
function archiveStep(steps) {
  return commandStep(steps, ARCHIVE_STEP_RUN)
}

test("85: ci.yml's sources job restores the archive cache right after its checkout and makes the archive on a miss", async () => {
  for (const { file, steps } of await cachingSourcesJobs()) {
    const lookup = idStep(steps, 'occt-source')
    const archive = steps[lookup + 1]

    assert.equal(lookup, 1, `${file}: the step occt-source does not come right after the checkout`)
    assert.equal(stepValue(steps[lookup], 'uses'), pinned('actions/cache'), file)
    assert.deepEqual(
      stepMapping(steps[lookup], 'with'),
      { path: ARCHIVE_CACHE_PATH, key: ARCHIVE_CACHE_KEY },
      file,
    )
    assert.ok(archive, `${file}: the sources job has no step after the step occt-source`)
    assert.equal(stepValue(archive, 'if'), "steps.occt-source.outputs.cache-hit != 'true'", file)
    assert.equal(stepValue(archive, 'shell'), undefined, file)
    assert.equal(stepValue(archive, 'run'), ARCHIVE_STEP_RUN, file)
    assert.ok(
      lookup + 1 < namedStep(steps, ARCHIVE_WRITER),
      `${file}: the archive step does not come before ${ARCHIVE_WRITER}`,
    )
  }
})

test("85: release.yml's sources job restores no archive cache and makes the archive right after its checkout in every run", async () => {
  const { steps } = (await sourcesJobs()).find(({ file }) => file === UNCACHED_ARCHIVE_FILE)
  const archive = archiveStep(steps)

  for (const action of ['actions/cache', 'actions/cache/restore']) {
    assert.ok(
      !steps.some((step) => usesAction(step, action)),
      `the sources job of ${UNCACHED_ARCHIVE_FILE} uses ${action}`,
    )
  }
  assert.equal(archive, 1, 'the archive step does not come right after the checkout')
  assert.equal(stepValue(steps[archive], 'if'), undefined)
  assert.equal(stepValue(steps[archive], 'shell'), undefined)
  assert.ok(
    archive < namedStep(steps, ARCHIVE_WRITER),
    `the archive step does not come before ${ARCHIVE_WRITER}`,
  )
})

test('85: no job but sources names the archive cache or runs the archive script, and platforms.yml has no sources job', async () => {
  for (const [file, text] of await workflowTexts(SOURCE_RULE_FILES)) {
    for (const job of workflowJobs(text).filter((listed) => listed.id !== SOURCES_JOB)) {
      for (const step of jobSteps(job)) {
        const stepText = step.join('\n')

        assert.ok(
          !stepText.includes(ARCHIVE_CACHE_PATH) && !stepText.includes(ARCHIVE_SCRIPT),
          `${file}: a step of ${job.id} names ${ARCHIVE_CACHE_PATH} or ${ARCHIVE_SCRIPT}`,
        )
      }
    }
  }
  assert.ok(
    !(await workflowJobIds('platforms.yml')).includes(SOURCES_JOB),
    `platforms.yml has a job ${SOURCES_JOB}`,
  )
})

test('85: web needs exactly core-wasm and sources', async () => {
  assert.deepEqual(jobNeeds(await workflowJob('ci.yml', 'web')), ['core-wasm', 'sources'])
})

test("86: each sources job writes the OCCT archive after the steps that make it and before Partsmith's, with one run", async () => {
  const jobs = await sourcesJobs()
  const runs = new Set()
  for (const { file, steps } of jobs) {
    const writer = namedStep(steps, ARCHIVE_WRITER)

    assert.ok(
      archiveStep(steps) < writer && writer < namedStep(steps, PARTSMITH_WRITER),
      `${file}: ${ARCHIVE_WRITER} is not between the archive step and ${PARTSMITH_WRITER}`,
    )
    assert.equal(stepValue(steps[writer], 'shell'), undefined, file)
    assert.equal(stepValue(steps[writer], 'working-directory'), undefined, file)
    runs.add(stepValue(steps[writer], 'run'))
  }
  assert.equal(runs.size, 1, `the run of ${ARCHIVE_WRITER} differs between the two files`)
})

/** The tag, commit and SHA-256 of the archive that the Measured paragraph of section 2.13 records. */
async function measuredArchive() {
  const match = MEASURED_ARCHIVE.exec(await repositoryText('docs/spec/delivery.md'))
  assert.ok(
    match,
    'docs/spec/delivery.md has no Measured paragraph with a tag, a commit and a SHA-256',
  )
  return { tag: match[1], commit: match[2], sha256: match[3] }
}

/** The SHA-256 of the archive that the `env` of the archive writer of ci.yml's sources job holds. */
async function writerSha256() {
  const steps = jobSteps(await workflowJob('ci.yml', SOURCES_JOB))
  return stepMapping(steps[namedStep(steps, ARCHIVE_WRITER)], 'env')?.OCCT_SOURCE_SHA256
}

test('86: each archive writer holds the SHA-256 that section 2.13 measured for the pinned tag and commit', async () => {
  const { tag, commit } = await readOcctPins()
  const measured = await measuredArchive()

  assert.deepEqual({ tag: measured.tag, commit: measured.commit }, { tag, commit })
  for (const { file, steps } of await sourcesJobs()) {
    assert.deepEqual(
      stepMapping(steps[namedStep(steps, ARCHIVE_WRITER)], 'env'),
      { OCCT_SOURCE_SHA256: measured.sha256 },
      file,
    )
  }
})

/** The SHA-256 of `bytes` in hexadecimal, as sha256sum prints it. */
function sha256Of(bytes) {
  return createHash('sha256').update(bytes).digest('hex')
}

/** `length` bytes that differ from one position to the next, as a stand-in for an archive or a part. */
function testBytes(length) {
  return Buffer.from(Array.from({ length }, (_, i) => i % 251))
}

/** The warning of section 4 for the archive `name` whose SHA-256 is `sha256`, not `expected`. */
function archiveChangedWarning(name, sha256, expected) {
  return `::warning title=OCCT source archive changed::${name} has the SHA-256 ${sha256}, not ${expected}, the one docs/spec/delivery.md section 2.13 records. A new pin, git or gzip wrote other bytes: record the new SHA-256 there and in OCCT_SOURCE_SHA256 of each step ${ARCHIVE_WRITER}.`
}

/** The cached-archive error of section 4 for the archive of OCCT `version` and the pinned commit `commit`. */
function cachedArchiveErrorMessage(version, commit) {
  return `::error title=OCCT source missing::${SOURCES_FOLDER}/${sourceArchiveName(version)} from the cache ${ARCHIVE_CACHE_PATH} is not a whole gzip file or does not hold the source of the OCCT commit ${commit}, so nothing was uploaded. Delete that cache entry (gh cache list --key occt-source-) and run the job again, which makes the archive anew (docs/spec/delivery.md section 2.13).`
}

/** The names of the files in `folder`, or none when it does not exist. */
async function filesIn(folder) {
  return readdir(folder).catch((error) => {
    if (error.code === 'ENOENT') {
      return []
    }
    throw error
  })
}

// The files of the tagged commit of the fixture of tests 86 and 87: a short text, and big.bin, whose tar is larger than a pipe
// holds.
const TAGGED_ARCHIVE_FILES = {
  'README.md': '# Open CASCADE Technology\n',
  'src/Standard/Standard.cxx': '// Standard, as tagged\n',
  'big.bin': Buffer.alloc(PIPE_BREAKING_SIZE),
}

/**
 * Writes `git archive --format=tar --prefix=occt-{version}/ <commit>` of `repository`, piped to `gzip -9 -n`, to
 * `path`, as scripts/occt-source-archive.sh packs its tag (section 2.13).
 */
function writeCommitArchive(environment, repository, version, commit, path) {
  const result = spawnSync(
    'bash',
    [
      '-c',
      'set -o pipefail; git archive --format=tar --prefix="$1" "$2" | gzip -9 -n > "$3"',
      'bash',
      `occt-${version}/`,
      commit,
      path,
    ],
    { cwd: repository, env: environment, encoding: 'utf8' },
  )
  assert.equal(result.status, 0, result.stderr)
}

/** Copies the script `script` of the repository to the same path in `folder`, as the checkout of a job holds it. */
async function copyScript(script, folder) {
  await mkdir(join(folder, 'scripts'), { recursive: true })
  await copyFile(join(repositoryRoot, script), join(folder, script))
  await chmod(join(folder, script), 0o755)
}

/**
 * Runs the step `name` of the deploy job with `bash -e` in a new folder that holds the files of `checkout` (path to
 * contents) and a copy of the check script, as the job's checkout does, after writing `files` into `ui/dist/` (name to
 * bytes; no `ui/dist` when `files` is undefined), in `environment`. With `ignorePipe`, the shell that starts the step
 * ignores SIGPIPE.
 */
async function runDeployCheck(context, name, { checkout = {}, files, environment, ignorePipe }) {
  const steps = await ciJobSteps('deploy')
  const workspace = await temporaryDirectory(context)
  await writeFiles(workspace, checkout)
  await copyScript(CHECK_SCRIPT, workspace)
  if (files !== undefined) {
    await mkdir(join(workspace, 'ui', 'dist'), { recursive: true })
    await writeFiles(join(workspace, 'ui', 'dist'), files)
  }
  return runStep(context, steps[namedStep(steps, name)], workspace, environment, { ignorePipe })
}

/**
 * Makes the fixture of tests 86 and 87 in a new folder: a repository whose first commit holds TAGGED_ARCHIVE_FILES and is
 * tagged `{tag}` (annotated), and whose second commit changes `src/Standard/Standard.cxx`, and the archives of both
 * commits. Resolves to the folder, the Git environment, the pins, both commits and both archives' paths.
 */
async function archiveCheckFixture(context) {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const { version, tag } = await readOcctPins()
  const repository = join(directory, 'repository')
  const { tagged, second } = await makeTaggedRepository(
    environment,
    repository,
    TAGGED_ARCHIVE_FILES,
    { version, tag },
  )
  const taggedArchive = join(directory, 'tagged.tar.gz')
  const secondArchive = join(directory, 'second.tar.gz')
  writeCommitArchive(environment, repository, version, tagged, taggedArchive)
  writeCommitArchive(environment, repository, version, second, secondArchive)
  return { directory, environment, version, tag, tagged, second, taggedArchive, secondArchive }
}

/**
 * Runs the archive writer of ci.yml's sources job with `bash -e` in a new folder `<root>` whose
 * scripts/occt-version.sh pins the fixture's version, its tag and its tagged commit, with a copy of the check script,
 * the files of `cached` (name to bytes) in `.occt/source/` and no `sources/`, in the fixture's Git environment with
 * `OCCT_SOURCE_SHA256` set to `expected`. Resolves to the step's result and the folder `<root>/sources`.
 */
async function runArchiveWriter(context, fixture, cached, expected) {
  const steps = jobSteps(await workflowJob('ci.yml', SOURCES_JOB))
  const root = await temporaryDirectory(context)
  await writeFiles(root, {
    [OCCT_PINS]: occtVersionText({
      version: fixture.version,
      tag: fixture.tag,
      commit: fixture.tagged,
    }),
  })
  await copyScript(CHECK_SCRIPT, root)
  await mkdir(join(root, ARCHIVE_CACHE_PATH), { recursive: true })
  await writeFiles(join(root, ARCHIVE_CACHE_PATH), cached)

  const result = await runStep(context, steps[namedStep(steps, ARCHIVE_WRITER)], root, {
    ...fixture.environment,
    OCCT_SOURCE_SHA256: expected,
  })
  return { result, sources: join(root, SOURCES_FOLDER) }
}

test('86: the archive writer copies the archive of the pinned OCCT commit into sources/ and prints its SHA-256', async (context) => {
  const fixture = await archiveCheckFixture(context)
  const name = sourceArchiveName(fixture.version)
  const archive = await readFile(fixture.taggedArchive)
  const sha256 = sha256Of(archive)

  const { result, sources } = await runArchiveWriter(context, fixture, { [name]: archive }, sha256)

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await filesIn(sources), [name])
  assert.deepEqual(await readFile(join(sources, name)), archive)
  assert.equal(result.stdout, `${sha256}  ${name}\n`)
  assert.equal(result.stderr, '')
})

test('86: the archive writer copies an archive with other bytes than section 2.13 records and warns without failing', async (context) => {
  const fixture = await archiveCheckFixture(context)
  const name = sourceArchiveName(fixture.version)
  const archive = await readFile(fixture.taggedArchive)
  const sha256 = sha256Of(archive)
  const expected = await writerSha256()

  const { result, sources } = await runArchiveWriter(
    context,
    fixture,
    { [name]: archive },
    expected,
  )

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await filesIn(sources), [name])
  assert.deepEqual(await readFile(join(sources, name)), archive)
  assert.equal(
    result.stdout,
    `${sha256}  ${name}\n${archiveChangedWarning(name, sha256, expected)}\n`,
  )
})

/** Asserts that the archive writer fails with exactly the cached-archive error when the cache holds `archive`. */
async function assertCachedArchiveRejected(context, fixture, archive) {
  const name = sourceArchiveName(fixture.version)

  const { result } = await runArchiveWriter(
    context,
    fixture,
    { [name]: archive },
    sha256Of(archive),
  )

  assert.equal(result.status, 1, result.stderr)
  assert.equal(result.stdout, `${cachedArchiveErrorMessage(fixture.version, fixture.tagged)}\n`)
  assert.equal(result.stderr, '')
}

test('86: the archive writer fails with the cached-archive error when the cache holds the archive of another commit', async (context) => {
  const fixture = await archiveCheckFixture(context)

  await assertCachedArchiveRejected(context, fixture, await readFile(fixture.secondArchive))
})

test("86: the archive writer fails with the cached-archive error when the cached archive lacks gzip's checksum and length", async (context) => {
  const fixture = await archiveCheckFixture(context)
  const cutOff = (await readFile(fixture.taggedArchive)).subarray(0, -8)

  await assertCachedArchiveRejected(context, fixture, cutOff)
})

test('86: the archive writer fails and writes nothing when the cache holds no archive', async (context) => {
  const fixture = await archiveCheckFixture(context)

  const { result, sources } = await runArchiveWriter(context, fixture, {}, await writerSha256())

  assert.notEqual(result.status, 0)
  assert.deepEqual(await filesIn(sources), [])
})

test("86: the archive writer fails and writes nothing when the cache holds only another version's archive", async (context) => {
  const fixture = await archiveCheckFixture(context)

  const { result, sources } = await runArchiveWriter(
    context,
    fixture,
    { [sourceArchiveName(OTHER_OCCT_VERSION)]: await readFile(fixture.taggedArchive) },
    await writerSha256(),
  )

  assert.notEqual(result.status, 0)
  assert.deepEqual(await filesIn(sources), [])
})

test('87: the deploy job checks the OCCT source archive after the source stamp and before Wrangler', async () => {
  const steps = await ciJobSteps('deploy')
  const stamp = namedStep(steps, 'Check the source stamp')
  const check = namedStep(steps, ARCHIVE_CHECK)

  assert.equal(stepValue(steps[check], 'if'), "steps.secrets.outputs.available == 'true'")
  assert.ok(
    stamp < check && check < idStep(steps, 'pages'),
    `${ARCHIVE_CHECK} is not between the source stamp check and Wrangler`,
  )
})

/** The OCCT-source error of section 4 for the archive of OCCT `version` and the pinned commit `commit`. */
function archiveErrorMessage(version, commit) {
  return `::error title=OCCT source missing::ui/dist/${sourceArchiveName(version)} is missing or does not hold the source of the OCCT commit ${commit}, so nothing was deployed. The sources job writes it, and the web job puts it into the build (docs/spec/delivery.md section 2.13).`
}

/**
 * Runs the deploy job's archive check in a new folder whose scripts/occt-version.sh pins the fixture's version, its tag
 * and its tagged commit, with a copy of the check script, after writing `files` into `ui/dist/` (name to bytes; no
 * `ui/dist` when `files` is undefined), in the fixture's Git environment. With `ignorePipe`, the shell that starts the
 * step ignores SIGPIPE.
 */
function runArchiveCheck(context, fixture, files, { ignorePipe = false } = {}) {
  return runDeployCheck(context, ARCHIVE_CHECK, {
    checkout: {
      [OCCT_PINS]: occtVersionText({
        version: fixture.version,
        tag: fixture.tag,
        commit: fixture.tagged,
      }),
    },
    files,
    environment: fixture.environment,
    ignorePipe,
  })
}

/** Asserts that the archive check fails with exactly the OCCT-source error, as `runArchiveCheck` takes `files`. */
async function assertArchiveRejected(context, fixture, files) {
  const result = await runArchiveCheck(context, fixture, files)

  assert.equal(result.status, 1, result.stderr)
  assert.equal(result.stdout, `${archiveErrorMessage(fixture.version, fixture.tagged)}\n`)
  assert.equal(result.stderr, '')
}

/** Runs `script` with `bash -c`, its arguments after it, in `environment`. */
function runShell(environment, script, ...scriptArguments) {
  return spawnSync('bash', ['-c', script, 'bash', ...scriptArguments], {
    env: environment,
    encoding: 'utf8',
  })
}

test('87: the archive check passes silently on the archive of the pinned commit, with SIGPIPE at its default and ignored', async (context) => {
  const fixture = await archiveCheckFixture(context)
  const files = { [sourceArchiveName(fixture.version)]: await readFile(fixture.taggedArchive) }

  // The fixture breaks the pipe: git exits after the first header while gzip is still writing.
  const brokenPipe = runShell(
    fixture.environment,
    `trap '' PIPE; gzip -dc "$1" | git get-tar-commit-id`,
    fixture.taggedArchive,
  )
  assert.equal(brokenPipe.stdout, `${fixture.tagged}\n`)
  assert.match(brokenPipe.stderr, /^gzip: stdout: Broken pipe$/m)

  const runs = [false, true]
  const results = await Promise.all(
    runs.map((ignorePipe) => runArchiveCheck(context, fixture, files, { ignorePipe })),
  )

  for (const [i, result] of results.entries()) {
    const label = `SIGPIPE ${runs[i] ? 'ignored' : 'at its default'}`
    assert.equal(result.status, 0, `${label}: ${result.stdout}${result.stderr}`)
    assert.equal(result.stdout, '', label)
    assert.equal(result.stderr, '', label)
  }
})

test('87: the archive check fails with the OCCT-source error when the archive is missing', async (context) => {
  await assertArchiveRejected(context, await archiveCheckFixture(context), {})
})

test('87: the archive check fails with the OCCT-source error when ui/dist is missing', async (context) => {
  await assertArchiveRejected(context, await archiveCheckFixture(context), undefined)
})

test('87: the archive check fails with the OCCT-source error when the archive holds another commit', async (context) => {
  const fixture = await archiveCheckFixture(context)

  await assertArchiveRejected(context, fixture, {
    [sourceArchiveName(fixture.version)]: await readFile(fixture.secondArchive),
  })
})

test("87: the archive check fails with the OCCT-source error when the archive lacks gzip's checksum and length", async (context) => {
  const fixture = await archiveCheckFixture(context)
  const cutOff = join(fixture.directory, 'cut-off.tar.gz')
  await writeFile(cutOff, (await readFile(fixture.taggedArchive)).subarray(0, -8))

  // Its first header still names the tagged commit. Only gzip -t tells it apart.
  assert.equal(
    runShell(fixture.environment, 'gzip -dc "$1" 2>/dev/null | git get-tar-commit-id', cutOff)
      .stdout,
    `${fixture.tagged}\n`,
  )
  await assertArchiveRejected(context, fixture, {
    [sourceArchiveName(fixture.version)]: await readFile(cutOff),
  })
})

test('87: the archive check fails with the OCCT-source error when the archive is a tar without gzip', async (context) => {
  const fixture = await archiveCheckFixture(context)

  await assertArchiveRejected(context, fixture, {
    [sourceArchiveName(fixture.version)]: gunzipSync(await readFile(fixture.taggedArchive)),
  })
})

test('87: the archive check fails with the OCCT-source error when the archive is empty', async (context) => {
  const fixture = await archiveCheckFixture(context)

  await assertArchiveRejected(context, fixture, { [sourceArchiveName(fixture.version)]: '' })
})

test('87: the archive check fails with the OCCT-source error when the archive is a whole gzip file whose tar names no commit', async (context) => {
  const fixture = await archiveCheckFixture(context)
  const folder = join(fixture.directory, 'plain')
  const plain = join(fixture.directory, 'plain.tar.gz')
  await writeFiles(join(folder, `occt-${fixture.version}`), TAGGED_ARCHIVE_FILES)
  const packed = runShell(
    fixture.environment,
    'tar -czf "$1" -C "$2" "$3"',
    plain,
    folder,
    `occt-${fixture.version}`,
  )
  assert.equal(packed.status, 0, packed.stderr)
  assert.equal(runShell(fixture.environment, 'gzip -t "$1"', plain).status, 0)

  await assertArchiveRejected(context, fixture, {
    [sourceArchiveName(fixture.version)]: await readFile(plain),
  })
})

test("87: the archive check fails with the OCCT-source error when only another version's archive exists", async (context) => {
  const fixture = await archiveCheckFixture(context)

  await assertArchiveRejected(context, fixture, {
    [sourceArchiveName(OTHER_OCCT_VERSION)]: await readFile(fixture.taggedArchive),
  })
})

test("93: the web sources job installs the toolchain, then writes the dependency archives after Partsmith's and before the upload", async () => {
  const webJobs = (await sourcesJobs()).filter(({ file }) => file !== 'release.yml')
  for (const { file, steps } of webJobs) {
    const toolchain = commandStep(steps, TOOLCHAIN_STEP)
    const partsmith = namedStep(steps, PARTSMITH_WRITER)
    const writer = namedStep(steps, DEPENDENCY_WRITER)
    const upload = usesStep(steps, 'actions/upload-artifact')

    assert.ok(
      toolchain < writer,
      `${file}: the toolchain is not installed before ${DEPENDENCY_WRITER}`,
    )
    assert.ok(
      partsmith < writer && writer < upload,
      `${file}: ${DEPENDENCY_WRITER} is not between ${PARTSMITH_WRITER} and the upload`,
    )
    assert.equal(stepValue(steps[writer], 'shell'), undefined, file)
    assert.equal(stepMapping(steps[writer], 'env'), undefined, file)
    assert.equal(stepValue(steps[writer], 'run'), DEPENDENCY_WRITER_RUN, file)
  }
})

// A stand-in for node that writes its arguments, one per line, to $NODE_CALLS, then writes WEB_PARTS with known bytes
// into the folder of its third argument, or fails when $NODE_FAILS is 1.
const NODE_STUB = `#!/usr/bin/env bash
printf '%s\n' "$@" > "$NODE_CALLS"
if [ "$NODE_FAILS" = 1 ]; then
  exit 1
fi
for part in ${WEB_PARTS.join(' ')}; do
  printf 'the bytes of %s\n' "$part" > "$3/$part"
done
`

/**
 * Runs the step `name` of the sources job of `file` with `bash -e` in a new folder `<root>` that holds an empty
 * `sources/`, with NODE_STUB first on PATH, failing when `fails` is set. Resolves to the step's result, the folder
 * `<root>/sources` and the arguments the stub received.
 */
async function runWithNodeStub(context, file, name, { fails = false } = {}) {
  const steps = jobSteps(await workflowJob(file, SOURCES_JOB))
  const root = await temporaryDirectory(context)
  const sources = join(root, SOURCES_FOLDER)
  const stubs = join(root, 'stubs')
  const calls = join(root, 'node-calls')
  await mkdir(sources)
  await mkdir(stubs)
  await writeFile(join(stubs, 'node'), NODE_STUB)
  await chmod(join(stubs, 'node'), 0o755)
  await writeFile(calls, '')

  const result = await runStep(context, steps[namedStep(steps, name)], root, {
    ...process.env,
    PATH: `${stubs}:${process.env.PATH}`,
    NODE_CALLS: calls,
    NODE_FAILS: fails ? '1' : '0',
  })
  return {
    result,
    sources,
    nodeArguments: (await readFile(calls, 'utf8')).split('\n').slice(0, -1),
  }
}

test('93: the dependency writer runs the script for the web scope and lists every part in SHA256SUMS in byte order', async (context) => {
  const { result, sources, nodeArguments } = await runWithNodeStub(
    context,
    'ci.yml',
    DEPENDENCY_WRITER,
  )

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(nodeArguments, [DEPENDENCY_SCRIPT, 'web', `${SOURCES_FOLDER}/dependencies`])
  const folder = join(sources, 'dependencies')
  assert.deepEqual((await readdir(folder)).toSorted(), [...WEB_PARTS, 'SHA256SUMS'].toSorted())
  assert.equal(
    await readFile(join(folder, 'SHA256SUMS'), 'utf8'),
    WEB_PARTS.map((part) => `${sha256Of(Buffer.from(`the bytes of ${part}\n`))}  ${part}\n`).join(
      '',
    ),
  )
})

test('93: the dependency writer fails and writes no SHA256SUMS when the script fails', async (context) => {
  const { result, sources } = await runWithNodeStub(context, 'ci.yml', DEPENDENCY_WRITER, {
    fails: true,
  })

  assert.notEqual(result.status, 0)
  assert.deepEqual(await filesIn(join(sources, 'dependencies')), [])
})

test('93: the deploy job checks the dependency archives after the OCCT archive and before Wrangler', async () => {
  const steps = await ciJobSteps('deploy')
  const occtCheck = namedStep(steps, ARCHIVE_CHECK)
  const check = namedStep(steps, DEPENDENCY_CHECK)

  assert.equal(stepValue(steps[check], 'if'), "steps.secrets.outputs.available == 'true'")
  assert.ok(
    occtCheck < check && check < idStep(steps, 'pages'),
    `${DEPENDENCY_CHECK} is not between ${ARCHIVE_CHECK} and Wrangler`,
  )
})

// The dependency-source error of section 4.
const DEPENDENCY_ERROR =
  '::error title=Dependency source missing::ui/dist/dependencies does not hold every part of rust-crates-web.tar.gz and npm-packages-web.tar with the SHA-256 that its SHA256SUMS lists, so nothing was deployed. The sources job writes them, and the web job puts them into the build (docs/spec/delivery.md section 2.13).'

// The byte at which the fixture's packages tar is cut into two parts: inside the data of its second file, b.bin, whose
// header takes bytes 3,584 to 4,095 (test 93). GNU tar reads a tar that ends inside a header as whole.
const NPM_PART_CUT = 6000

/**
 * The parts of the web archives of the deploy check's fixture, as name to bytes: a whole gzip file as the crates'
 * only part, and a tar of two files, a.bin of 3,000 bytes and b.bin of 5,000, cut at NPM_PART_CUT into two parts.
 */
async function webParts(context) {
  const folder = await temporaryDirectory(context)
  await writeFiles(folder, { 'a.bin': testBytes(3000), 'b.bin': testBytes(5000) })
  const packed = spawnSync('tar', ['-cf', '-', '-C', folder, 'a.bin', 'b.bin'])
  assert.equal(packed.status, 0, String(packed.stderr))
  const tar = packed.stdout
  return {
    'rust-crates-web.tar.gz.part-00': gzipSync(testBytes(2000)),
    'npm-packages-web.tar.part-00': tar.subarray(0, NPM_PART_CUT),
    'npm-packages-web.tar.part-01': tar.subarray(NPM_PART_CUT),
  }
}

/** The SHA256SUMS text that `sha256sum -- *` writes for `parts` (name to bytes), in byte order of the names. */
function sha256Sums(parts) {
  return Object.keys(parts)
    .toSorted()
    .map((name) => `${sha256Of(parts[name])}  ${name}\n`)
    .join('')
}

/**
 * Runs the deploy job's dependency check in a new folder after writing `files` into `ui/dist/dependencies/` (name to
 * bytes; no such folder when `files` is undefined).
 */
async function runDependencyCheck(context, files) {
  const steps = await ciJobSteps('deploy')
  const workspace = await temporaryDirectory(context)
  if (files !== undefined) {
    const folder = join(workspace, 'ui', 'dist', 'dependencies')
    await mkdir(folder, { recursive: true })
    await writeFiles(folder, files)
  }
  return runStep(context, steps[namedStep(steps, DEPENDENCY_CHECK)], workspace, process.env)
}

/** Asserts that the dependency check fails with exactly the dependency-source error for `files`. */
async function assertDependenciesRejected(context, files) {
  const result = await runDependencyCheck(context, files)

  assert.equal(result.status, 1, result.stderr)
  assert.equal(result.stdout, `${DEPENDENCY_ERROR}\n`)
  assert.equal(result.stderr, '')
}

/** `parts` without `name`, with the SHA256SUMS written for what is left. */
function withoutPart(parts, name) {
  const left = Object.fromEntries(Object.entries(parts).filter(([listed]) => listed !== name))
  return { ...left, SHA256SUMS: sha256Sums(left) }
}

/** `parts` with the part `name` cut to its first `length` bytes, and SHA256SUMS written again for them. */
function withCutPart(parts, name, length) {
  const cut = { ...parts, [name]: parts[name].subarray(0, length) }
  return { ...cut, SHA256SUMS: sha256Sums(cut) }
}

test('93: the dependency check passes silently when every part matches SHA256SUMS and the parts join into whole archives', async (context) => {
  const parts = await webParts(context)

  const result = await runDependencyCheck(context, { ...parts, SHA256SUMS: sha256Sums(parts) })

  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`)
  assert.equal(result.stdout, '')
  assert.equal(result.stderr, '')
})

test('93: the dependency check fails when ui/dist/dependencies or its SHA256SUMS is missing', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, undefined)
  await assertDependenciesRejected(context, parts)
})

test('93: the dependency check fails when a part has one byte changed', async (context) => {
  const parts = await webParts(context)
  const changed = Buffer.from(parts['npm-packages-web.tar.part-01'])
  changed[10] ^= 0xff

  await assertDependenciesRejected(context, {
    ...parts,
    'npm-packages-web.tar.part-01': changed,
    SHA256SUMS: sha256Sums(parts),
  })
})

test('93: the dependency check fails when a part that SHA256SUMS lists is missing', async (context) => {
  const parts = await webParts(context)
  const { 'npm-packages-web.tar.part-01': _missing, ...left } = parts

  await assertDependenciesRejected(context, { ...left, SHA256SUMS: sha256Sums(parts) })
})

test('93: the dependency check fails when SHA256SUMS lacks the first part of either archive', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, withoutPart(parts, 'rust-crates-web.tar.gz.part-00'))
  await assertDependenciesRejected(context, withoutPart(parts, 'npm-packages-web.tar.part-00'))
})

test('93: the dependency check fails when SHA256SUMS holds a line that is not in the format of sha256sum', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, {
    ...parts,
    SHA256SUMS: `${sha256Sums(parts)}not a checksum line\n`,
  })
})

test('93: the dependency check fails when the last packages part is missing from both the folder and SHA256SUMS', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, withoutPart(parts, 'npm-packages-web.tar.part-01'))
})

test('93: the dependency check fails when a part is cut short and SHA256SUMS is written again for it', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, withCutPart(parts, 'npm-packages-web.tar.part-01', 100))
  await assertDependenciesRejected(
    context,
    withCutPart(
      parts,
      'rust-crates-web.tar.gz.part-00',
      parts['rust-crates-web.tar.gz.part-00'].length - 8,
    ),
  )
})

test('93: the dependency check fails when the folder holds a part that SHA256SUMS does not list', async (context) => {
  const parts = await webParts(context)

  await assertDependenciesRejected(context, {
    ...parts,
    'npm-packages-web.tar.part-02': testBytes(10),
    SHA256SUMS: sha256Sums(parts),
  })
})

test("94: the release's sources job installs the toolchain and Node, then writes the desktop archives after Partsmith's and before the upload", async () => {
  const steps = jobSteps(await workflowJob('release.yml', SOURCES_JOB))
  const checkout = usesStep(steps, 'actions/checkout')
  const toolchain = commandStep(steps, TOOLCHAIN_STEP)
  const node = usesStep(steps, 'actions/setup-node')
  const partsmith = namedStep(steps, PARTSMITH_WRITER)
  const writer = namedStep(steps, DEPENDENCY_WRITER)
  const upload = usesStep(steps, 'actions/upload-artifact')

  assert.equal(stepValue(steps[writer], 'run'), DEPENDENCY_RELEASE_RUN)
  assert.equal(stepValue(steps[writer], 'shell'), undefined)
  assert.equal(stepMapping(steps[writer], 'env'), undefined)
  assert.ok(
    partsmith < writer && writer < upload,
    `${DEPENDENCY_WRITER} is not between ${PARTSMITH_WRITER} and the upload`,
  )
  assert.ok(
    checkout < toolchain && toolchain < writer,
    'the toolchain is not installed between the checkout and the dependency archives',
  )
  assert.ok(
    checkout < node && node < writer,
    'Node is not set up between the checkout and the dependency archives',
  )
  assert.deepEqual(stepMapping(steps[node], 'with'), { 'node-version': '22' })
})

test("94: the release's dependency writer runs the script for the desktop scope into sources/", async (context) => {
  const passed = await runWithNodeStub(context, 'release.yml', DEPENDENCY_WRITER)
  assert.equal(passed.result.status, 0, passed.result.stderr)
  assert.deepEqual(passed.nodeArguments, [DEPENDENCY_SCRIPT, 'desktop', SOURCES_FOLDER])

  const failed = await runWithNodeStub(context, 'release.yml', DEPENDENCY_WRITER, { fails: true })
  assert.notEqual(failed.result.status, 0)
})

test('94: release.yml has no dry-run job of its own for the dependency archives', async () => {
  const jobs = workflowJobs(await workflowText('release.yml'))

  assert.ok(
    !jobs.some((job) => job.id === DEPENDENCY_DRY_RUN),
    `release.yml still has the job ${DEPENDENCY_DRY_RUN}`,
  )
})

test('94: the dependency script is a regular file, and pnpm lint and pnpm format reach it from the repository root', async () => {
  const status = await lstat(join(repositoryRoot, DEPENDENCY_SCRIPT))
  const scripts = JSON.parse(await repositoryText('ui/package.json')).scripts

  assert.ok(status.isFile(), `${DEPENDENCY_SCRIPT} is not a regular file`)
  assert.equal(scripts.lint, LINT_SCRIPT)
  assert.equal(scripts.format, FORMAT_SCRIPT)
})

test('95: the notices statements name the dependency archives, the folder of each deployment and the script', async () => {
  const { archiveNames } = await import(pathToFileURL(join(repositoryRoot, DEPENDENCY_SCRIPT)).href)
  const web = await repositoryText('ui/public/THIRD-PARTY-NOTICES.txt')
  const desktop = await repositoryText('THIRD-PARTY-NOTICES.txt')

  for (const name of [
    ...archiveNames('web'),
    '/dependencies/',
    '/dependencies/SHA256SUMS',
    DEPENDENCY_SCRIPT,
  ]) {
    assert.ok(web.includes(name), `ui/public/THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
  for (const name of [...archiveNames('desktop'), DEPENDENCY_SCRIPT]) {
    assert.ok(desktop.includes(name), `THIRD-PARTY-NOTICES.txt does not name ${name}`)
  }
})

test('95: the statements name the folder of the native sources, and the desktop one installs tauri-cli from the archive', async () => {
  const { TAURI_CLI } = await import(pathToFileURL(join(repositoryRoot, DEPENDENCY_SCRIPT)).href)
  const web = await repositoryText('ui/public/THIRD-PARTY-NOTICES.txt')
  const desktop = await repositoryText('THIRD-PARTY-NOTICES.txt')
  const install = `cargo install tauri-cli --version ${TAURI_CLI.version} --locked`

  assert.ok(
    web.includes('npm-sources-web'),
    'ui/public/THIRD-PARTY-NOTICES.txt does not name npm-sources-web',
  )
  assert.ok(
    desktop.includes('npm-sources-desktop'),
    'THIRD-PARTY-NOTICES.txt does not name npm-sources-desktop',
  )
  assert.ok(
    desktop.split('\n').some((line) => line.includes(install)),
    `THIRD-PARTY-NOTICES.txt does not hold ${install} on one line`,
  )
})

test("95: the dependency writer's folder is dependencies/ of sources-web, which web downloads into ui/dist, the deployment's root", async () => {
  const sources = jobSteps(await workflowJob('ci.yml', SOURCES_JOB))
  const run = stepValue(sources[namedStep(sources, DEPENDENCY_WRITER)], 'run')
  const upload = stepMapping(sources[usesStep(sources, 'actions/upload-artifact')], 'with')
  const web = await ciJobSteps('web')

  assert.ok(
    run.split('\n').includes(`mkdir ${SOURCES_FOLDER}/dependencies`),
    `${DEPENDENCY_WRITER} does not make ${SOURCES_FOLDER}/dependencies`,
  )
  assert.equal(upload?.path, `${SOURCES_FOLDER}/`)
  stepIndex(web, 'in web that downloads sources-web into ui/dist', (step) =>
    downloadsArtifact(step, 'sources-web', 'ui/dist'),
  )
  assert.ok((await repositoryText('ui/public/THIRD-PARTY-NOTICES.txt')).includes('/dependencies/'))
})

/** The steps of rust-platforms in ci.yml and of the bundle job in release.yml, as `{ platforms, release }`. */
async function importCheckJobs() {
  const [platforms, release] = await Promise.all([
    workflowJob('ci.yml', 'rust-platforms'),
    workflowJob('release.yml', 'bundle'),
  ])
  return { platforms: jobSteps(platforms), release: jobSteps(release) }
}

test('100: rust-platforms checks the imports of the debug app on Windows after clippy, also after failing crate tests', async () => {
  const { platforms: steps } = await importCheckJobs()
  const clippy = commandStep(steps, PLATFORM_CLIPPY)
  const check = namedStep(steps, IMPORT_CHECK)

  assert.ok(clippy < check, `${IMPORT_CHECK} comes before clippy`)
  assert.equal(stepValue(steps[check], 'if'), IMPORT_CHECK_CONDITION)
  assert.equal(stepValue(steps[check], 'run'), importCheckRun(DEBUG_APP))
})

test('100: the Windows bundle job checks the imports of the app the installer holds, right after cargo tauri build', async () => {
  const { release: steps } = await importCheckJobs()
  const build = commandStep(steps, TAURI_BUILD)
  const check = namedStep(steps, IMPORT_CHECK)
  const collect = namedStep(steps, 'Collect the bundles')

  assert.equal(check, build + 1, `${IMPORT_CHECK} is not right after cargo tauri build`)
  assert.equal(collect, check + 1, `${IMPORT_CHECK} is not right before Collect the bundles`)
  assert.equal(stepValue(steps[check], 'if'), "runner.os == 'Windows'")
  assert.equal(stepValue(steps[check], 'run'), importCheckRun(RELEASE_APP))
})

/** Asserts that the import check `check`, named `name`, runs in its job's shell and folder and may not fail. */
function assertPlainImportCheck(check, name) {
  for (const key of ['env', 'shell', 'working-directory', 'continue-on-error']) {
    assert.equal(stepValue(check, key), undefined, `${name} sets ${key}`)
  }
}

test("100: both import checks run in their job's shell and folder, with no env, and may not fail", async () => {
  const { platforms, release } = await importCheckJobs()

  assertPlainImportCheck(platforms[namedStep(platforms, IMPORT_CHECK)], IMPORT_CHECK)
  assertPlainImportCheck(release[namedStep(release, IMPORT_CHECK)], IMPORT_CHECK)
})

test('100: each job sets up the MSVC developer environment on Windows before its import check', async () => {
  for (const [job, steps] of Object.entries(await importCheckJobs())) {
    const msvc = usesStep(steps, 'ilammy/msvc-dev-cmd')

    assert.equal(stepValue(steps[msvc], 'if'), "runner.os == 'Windows'", job)
    assert.ok(msvc < namedStep(steps, IMPORT_CHECK), `${job}: MSVC is set up after ${IMPORT_CHECK}`)
  }
})

test('100: no step of the workflows runs dumpbin itself: the import script is the one copy of the check', async () => {
  for (const [file, text] of await workflowTexts(WORKFLOW_FILES)) {
    for (const job of workflowJobs(text)) {
      for (const step of jobSteps(job)) {
        assert.ok(
          !(stepValue(step, 'run') ?? '').includes('dumpbin'),
          `${file}: a step of ${job.id} runs dumpbin`,
        )
      }
    }
  }
})

/** The lines of a section of a dumpbin listing: its heading, then `names`, one per line, each set off by a blank line. */
function dumpbinSection(heading, names) {
  return [`  ${heading}`, '', ...names.map((name) => `    ${name}`), '']
}

/**
 * The listing of `dumpbin -nologo -dependents` for the program `executable`, by default the debug app, that imports
 * `dependencies` and, when `delayLoaded` is not empty, delay-loads it under the second heading, with the CR LF line
 * ends dumpbin writes.
 */
function dumpbinListing(dependencies, { delayLoaded = [], executable = DEBUG_APP } = {}) {
  return [
    '',
    `Dump of file ${executable}`,
    '',
    'File Type: EXECUTABLE IMAGE',
    '',
    ...dumpbinSection('Image has the following dependencies:', dependencies),
    ...(delayLoaded.length > 0
      ? dumpbinSection('Image has the following delay load dependencies:', delayLoaded)
      : []),
    '  Summary',
    '',
    '        5000 .data',
    '       2E000 .pdata',
    '      1B6000 .rdata',
    '        7000 .reloc',
    '      43C000 .text',
    '',
  ].join('\r\n')
}

/** The error of section 4 for the program `executable` that imports the Visual C++ runtime DLLs `names`, in order. */
function runtimeImportedMessage(executable, names) {
  return `::error title=Visual C++ runtime imported::${executable} imports ${names.join(', ')}, which only the Visual C++ Redistributable installs. On Windows, OCCT, the shim and every Rust crate link the Visual C++ runtime and the C++ standard library statically (docs/spec/delivery.md section 3.6).`
}

/** The error of section 4 for the program `executable` whose imports dumpbin did not list. */
function importsNotCheckedMessage(executable) {
  return `::error title=Imports not checked::dumpbin listed no imports of ${executable}, so the C runtime it imports was not checked. The steps that set up MSVC and build the program run before this one (docs/spec/delivery.md section 3.6).`
}

/** The error of section 4 for the program `executable` that imports no DLL of the Universal C Runtime of Windows. */
function ucrtNotImportedMessage(executable) {
  return `::error title=Universal C Runtime not imported::${executable} imports no api-ms-win-crt DLL. Every Windows program takes the Universal C Runtime from Windows through ucrt.lib, and .cargo/config.toml links ucrt.lib in place of the static libucrt.lib (docs/spec/delivery.md section 3.6).`
}

/**
 * Runs the import check `step` of rust-platforms, by default the app's, as GitHub runs a `bash` step, in a new folder
 * that holds a copy of the import script, as the job's checkout does, with the dumpbin stub first on `PATH`, printing
 * `listing` and exiting with `status`; with `listing` null, no dumpbin is on `PATH`. Resolves to the result and the
 * argument lists dumpbin received.
 */
async function runImportCheck(context, listing, { status = 0, step = IMPORT_CHECK } = {}) {
  const { platforms: steps } = await importCheckJobs()
  const directory = await temporaryDirectory(context)
  const workspace = join(directory, 'workspace')
  const stubDirectory = join(directory, 'stub')
  const callsPath = join(directory, 'calls')
  const listingPath = join(directory, 'listing')
  await Promise.all([
    copyScript(IMPORT_SCRIPT, workspace),
    mkdir(stubDirectory),
    writeFile(callsPath, ''),
  ])
  const environment = { ...process.env }
  if (listing !== null) {
    await writeFile(join(stubDirectory, 'dumpbin'), DUMPBIN_STUB)
    await chmod(join(stubDirectory, 'dumpbin'), 0o755)
    await writeFile(listingPath, listing)
    Object.assign(environment, {
      PATH: `${stubDirectory}:${process.env.PATH}`,
      DUMPBIN_CALLS: callsPath,
      DUMPBIN_LISTING: listingPath,
      DUMPBIN_STATUS: String(status),
    })
  }
  const result = await runStep(context, steps[namedStep(steps, step)], workspace, environment, {
    pipefail: true,
  })
  return { ...result, calls: await recordedCalls(callsPath) }
}

/** The last line of `text`, without its line break. */
function lastLine(text) {
  return text.replace(/\n$/, '').split('\n').at(-1)
}

/** Asserts that the import check passed: it exited 0 and printed no error. */
function assertImportsAccepted(result) {
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`)
  assert.doesNotMatch(result.stdout, /^::error/m)
}

/** Asserts that the import check exited 1 and that the last line of its standard output is `message`. */
function assertImportCheckFailed(result, message) {
  assert.equal(result.status, 1, `${result.stdout}${result.stderr}`)
  assert.equal(lastLine(result.stdout), message)
}

test('101: the import check passes on an app that takes its C runtime from Windows, after dumpbin -nologo -dependents', async (context) => {
  const result = await runImportCheck(context, dumpbinListing(WINDOWS_IMPORTS))

  assertImportsAccepted(result)
  assert.deepEqual(result.calls, [['-nologo', '-dependents', DEBUG_APP]])
  for (const name of WINDOWS_IMPORTS) {
    assert.ok(result.stdout.includes(name), `the listing printed lacks ${name}`)
  }
})

test('101: the import check prints the listing without its carriage returns', async (context) => {
  const result = await runImportCheck(context, dumpbinListing(WINDOWS_IMPORTS))

  assert.ok(result.stdout.includes(`    ${WINDOWS_IMPORTS[0]}\n`), result.stdout)
  assert.ok(!result.stdout.includes('\r'), 'the listing printed keeps a carriage return')
})

test('101: the import check passes on msvcp_win.dll and msvcrt.dll, which are part of Windows', async (context) => {
  const result = await runImportCheck(
    context,
    dumpbinListing([...WINDOWS_IMPORTS, 'msvcp_win.dll', 'msvcrt.dll']),
  )

  assertImportsAccepted(result)
})

test('101: the import check fails on MSVCP140.dll and VCRUNTIME140.dll, naming each runtime DLL in the order of the listing', async (context) => {
  const [kernel, user, ...rest] = WINDOWS_IMPORTS
  const listing = dumpbinListing([
    kernel,
    user,
    'MSVCP140.dll',
    ...rest,
    'VCRUNTIME140.dll',
    'VCRUNTIME140_1.dll',
  ])

  const result = await runImportCheck(context, listing)

  assertImportCheckFailed(
    result,
    runtimeImportedMessage(DEBUG_APP, ['MSVCP140.dll', 'VCRUNTIME140.dll', 'VCRUNTIME140_1.dll']),
  )
})

for (const name of OTHER_REDISTRIBUTABLE_DLLS) {
  test(`101: the import check fails on ${name}, a DLL of the Visual C++ Redistributable`, async (context) => {
    const result = await runImportCheck(context, dumpbinListing([...WINDOWS_IMPORTS, name]))

    assertImportCheckFailed(result, runtimeImportedMessage(DEBUG_APP, [name]))
  })
}

test('101: the import check fails on runtime DLLs that the app loads with a delay', async (context) => {
  const listing = dumpbinListing(WINDOWS_IMPORTS, {
    delayLoaded: ['vcruntime140.dll', 'VCOMP140.DLL'],
  })

  const result = await runImportCheck(context, listing)

  assertImportCheckFailed(
    result,
    runtimeImportedMessage(DEBUG_APP, ['vcruntime140.dll', 'VCOMP140.DLL']),
  )
})

for (const status of [1, 0]) {
  test(`101: the import check fails with Imports not checked when dumpbin cannot open the app and exits ${status}`, async (context) => {
    const result = await runImportCheck(context, unreadableListing(DEBUG_APP), { status })

    assertImportCheckFailed(result, importsNotCheckedMessage(DEBUG_APP))
  })
}

test('101: the import check fails with Imports not checked when dumpbin is not on PATH', async (context) => {
  const lookup = spawnSync('bash', ['-c', 'command -v dumpbin'], { encoding: 'utf8' })
  assert.notEqual(lookup.status, 0, `this machine has a dumpbin on PATH: ${lookup.stdout}`)

  const result = await runImportCheck(context, null)

  assertImportCheckFailed(result, importsNotCheckedMessage(DEBUG_APP))
})

test('101: the import script exits 2 with its usage line for other than one argument', () => {
  for (const scriptArguments of [[], [DEBUG_APP, RELEASE_APP]]) {
    const result = spawnSync('bash', [join(repositoryRoot, IMPORT_SCRIPT), ...scriptArguments], {
      encoding: 'utf8',
    })

    assert.equal(result.status, 2, `${scriptArguments.length} arguments`)
    assert.equal(result.stdout, '')
    assert.equal(result.stderr, `${IMPORT_USAGE}\n`)
  }
})

test('101: the import script is executable, since each import check runs it by its path', async () => {
  const { mode } = await lstat(join(repositoryRoot, IMPORT_SCRIPT))

  assert.equal(mode & 0o111, 0o111, `${IMPORT_SCRIPT} has the mode ${(mode & 0o777).toString(8)}`)
})

test('123: every listing of test 101 that passes imports the two api-ms-win-crt DLLs', () => {
  for (const name of ['api-ms-win-crt-runtime-l1-1-0.dll', 'api-ms-win-crt-heap-l1-1-0.dll']) {
    assert.ok(WINDOWS_IMPORTS.includes(name), `WINDOWS_IMPORTS lacks ${name}`)
  }
})

test('123: the import check fails with Universal C Runtime not imported on a program that imports no api-ms-win-crt DLL', async (context) => {
  const result = await runImportCheck(context, dumpbinListing(SYSTEM_IMPORTS))

  assertImportCheckFailed(result, ucrtNotImportedMessage(DEBUG_APP))
})

for (const name of ['api-ms-win-crt-runtime-l1-1-0.dll', 'API-MS-WIN-CRT-RUNTIME-L1-1-0.DLL']) {
  test(`123: the import check passes on a program that imports ${name}`, async (context) => {
    const result = await runImportCheck(context, dumpbinListing([...SYSTEM_IMPORTS, name]))

    assertImportsAccepted(result)
  })
}

test("123: the import check does not count the API sets of Windows' core as the Universal C Runtime", async (context) => {
  const result = await runImportCheck(
    context,
    dumpbinListing([...SYSTEM_IMPORTS, ...CORE_API_SETS]),
  )

  assertImportCheckFailed(result, ucrtNotImportedMessage(DEBUG_APP))
})

test('123: the import check asks for the API sets of ucrt.lib, not ucrtbase.dll itself', async (context) => {
  const result = await runImportCheck(context, dumpbinListing([...SYSTEM_IMPORTS, 'ucrtbase.dll']))

  assertImportCheckFailed(result, ucrtNotImportedMessage(DEBUG_APP))
})

test('123: the import check names the Visual C++ runtime DLLs before it looks for the Universal C Runtime', async (context) => {
  const listing = dumpbinListing([...SYSTEM_IMPORTS, 'MSVCP140.dll', 'VCRUNTIME140.dll'])

  const result = await runImportCheck(context, listing)

  assertImportCheckFailed(
    result,
    runtimeImportedMessage(DEBUG_APP, ['MSVCP140.dll', 'VCRUNTIME140.dll']),
  )
})

test('123: the import check passes on the listing of the debug app in platforms run 14', async (context) => {
  const result = await runImportCheck(context, dumpbinListing(RUN_14_APP_IMPORTS))

  assertImportsAccepted(result)
})

test('124: rust-platforms checks the imports of the CLI right after those of the app, on the same condition', async () => {
  const { platforms: steps } = await importCheckJobs()
  const app = namedStep(steps, IMPORT_CHECK)
  const cli = namedStep(steps, CLI_IMPORT_CHECK)

  assert.equal(cli, app + 1, `${CLI_IMPORT_CHECK} is not right after ${IMPORT_CHECK}`)
  assert.equal(
    namedStep(steps, DISK_SPACE),
    cli + 1,
    `${CLI_IMPORT_CHECK} is not right before ${DISK_SPACE}`,
  )
  assert.equal(stepValue(steps[cli], 'if'), IMPORT_CHECK_CONDITION)
  assert.equal(stepValue(steps[cli], 'run'), importCheckRun(DEBUG_CLI))
})

test("124: the CLI's check runs in its job's shell and folder, with no env, and may not fail", async () => {
  const { platforms } = await importCheckJobs()

  assertPlainImportCheck(platforms[namedStep(platforms, CLI_IMPORT_CHECK)], CLI_IMPORT_CHECK)
})

test('124: no job of the release workflow checks the imports of the CLI', async () => {
  for (const job of workflowJobs(await workflowText('release.yml'))) {
    const names = jobSteps(job).map((step) => stepValue(step, 'name'))
    assert.equal(
      names.includes(CLI_IMPORT_CHECK),
      false,
      `release.yml ${job.id} has ${CLI_IMPORT_CHECK}`,
    )
  }
})

test('124: the CLI the check reads is the binary partsmith of partsmith-cli', async () => {
  const manifest = await repositoryText('crates/partsmith-cli/Cargo.toml')
  // The name of each [[bin]] table, read up to the next table.
  const binaries = manifest
    .split(/^\[\[bin\]\]$/m)
    .slice(1)
    .map((table) => /^name = "([^"]+)"$/m.exec(table.split(/^\[/m)[0])?.[1])
  const checked = /^target\/debug\/(.+)\.exe$/.exec(DEBUG_CLI)?.[1]

  assert.equal(checked, 'partsmith')
  assert.ok(
    binaries.includes(checked),
    `crates/partsmith-cli/Cargo.toml names the binaries ${binaries}`,
  )
})

test("124: the CLI's check passes on a CLI that imports the Universal C Runtime of Windows, after dumpbin -nologo -dependents", async (context) => {
  const listing = dumpbinListing(
    ['KERNEL32.dll', 'api-ms-win-crt-runtime-l1-1-0.dll', 'api-ms-win-crt-math-l1-1-0.dll'],
    { executable: DEBUG_CLI },
  )

  const result = await runImportCheck(context, listing, { step: CLI_IMPORT_CHECK })

  assertImportsAccepted(result)
  assert.deepEqual(result.calls, [['-nologo', '-dependents', DEBUG_CLI]])
})

test("124: the CLI's check fails with Universal C Runtime not imported on a CLI that links it statically", async (context) => {
  const listing = dumpbinListing(['KERNEL32.dll'], { executable: DEBUG_CLI })

  const result = await runImportCheck(context, listing, { step: CLI_IMPORT_CHECK })

  assertImportCheckFailed(result, ucrtNotImportedMessage(DEBUG_CLI))
})

test("124: the CLI's check fails with Imports not checked, naming the program, when dumpbin cannot open it", async (context) => {
  const result = await runImportCheck(context, unreadableListing(DEBUG_CLI), {
    step: CLI_IMPORT_CHECK,
  })

  assertImportCheckFailed(result, importsNotCheckedMessage(DEBUG_CLI))
  assert.ok(lastLine(result.stdout).includes('build the program run before this one'))
})

test('104: full-rebuild runs on macos-15 right after the copy of the macOS OCCT row, for the macOS of OCCT and the app', async () => {
  const jobs = workflowJobs(await workflowText('platforms.yml'))
  const ids = jobs.map((job) => job.id)
  const job = findJob(jobs, 'platforms.yml', FULL_REBUILD)

  assert.equal(ids.indexOf(FULL_REBUILD), ids.indexOf('occt-platforms') + 1)
  assert.equal(jobValue(job, 'name'), 'Full rebuild of bench50 (macos-15)')
  assert.deepEqual(jobNeeds(job), ['occt-platforms'])
  assert.equal(jobValue(job, 'if'), '${{ !cancelled() }}')
  assert.equal(jobValue(job, 'runs-on'), 'macos-15')
  assert.equal(jobBlock(job, 'strategy'), undefined)
  assert.deepEqual(jobBlock(job, 'defaults'), ['run:', '  shell: bash'])
  assert.deepEqual(mappingEntries(jobBlock(job, 'env')), { MACOSX_DEPLOYMENT_TARGET: '"13.0"' })
})

test('104: platforms.yml keeps the macos-15 row of occt-platforms that full-rebuild needs, and that row alone', async () => {
  const jobs = workflowJobs(await workflowText('platforms.yml'))
  const runners = jobRunners(findJob(jobs, 'platforms.yml', 'occt-platforms'))

  assert.deepEqual(runners, ['macos-15'])
})

test('104: full-rebuild restores OCCT, sets up Rust and its own Rust cache, then times the full rebuild, in five steps', async () => {
  const steps = jobSteps(await workflowJob('platforms.yml', FULL_REBUILD))
  const rustSteps = jobSteps(await workflowJob('ci.yml', 'rust-platforms'))
  const [checkout, occt, toolchain, cache, timing] = steps

  assert.equal(steps.length, 5)
  assert.equal(stepValue(checkout, 'uses'), pinned('actions/checkout'))
  assert.equal(stepValue(occt, 'uses'), pinned('actions/cache/restore'))
  assert.deepEqual(stepMapping(occt, 'with'), {
    path: '.occt/native',
    key: await bundleOcctKey(),
    'fail-on-cache-miss': 'true',
  })
  assert.equal(stepValue(toolchain, 'run'), TOOLCHAIN_STEP)
  assert.equal(stepValue(cache, 'uses'), pinned('Swatinem/rust-cache'))
  assert.deepEqual(
    stepMapping(cache, 'with'),
    stepMapping(rustSteps[usesStep(rustSteps, 'Swatinem/rust-cache')], 'with'),
  )
  assert.equal(stepValue(timing, 'name'), FULL_REBUILD_STEP)
  assert.deepEqual(stepMapping(timing, 'env'), { OPERATION: FULL_REBUILD_OPERATION })
  for (const key of ['if', 'shell', 'working-directory', 'timeout-minutes', 'continue-on-error']) {
    assert.equal(stepValue(timing, key), undefined, `${FULL_REBUILD_STEP} sets ${key}`)
  }
})

test('104: the step reads a row that the recompute benchmark names and partsmith-budgets lays out', async () => {
  const [benchmark, report] = await Promise.all([
    repositoryText(RECOMPUTE_BENCHMARK),
    repositoryText(BUDGETS_REPORT),
  ])

  assert.ok(
    benchmark.includes(`"${FULL_REBUILD_OPERATION}"`),
    `${RECOMPUTE_BENCHMARK} names no row "${FULL_REBUILD_OPERATION}"`,
  )
  for (const format of ['"{:<width$}  {:>12}   budget {:>12}"', '"Over budget: {} ({} > {})"']) {
    assert.ok(report.includes(format), `${BUDGETS_REPORT} lays out no line as ${format}`)
  }
})

/** The row that `Report` of partsmith-budgets prints for the full rebuild alone, with `median` against `budget`. */
function fullRebuildRow(median, budget) {
  return `${FULL_REBUILD_OPERATION}  ${median.padStart(12)}   budget ${budget.padStart(12)}`
}

/** The job summary of section 2.14 for `median` against `budget` with `verdict`, on the macOS arm64 runner. */
function fullRebuildSummary(median, budget, verdict) {
  return [
    '## Full rebuild of bench50',
    '',
    '| Runner | Operation | Median | Budget |',
    '| --- | --- | --- | --- |',
    `| macOS ARM64 | ${FULL_REBUILD_OPERATION} | ${median} | ${budget} |`,
    '',
    verdict,
    '',
  ].join('\n')
}

/** The warning of section 4 for a full rebuild whose `median` is over its `budget`. */
function overBudgetWarning(median, budget) {
  return `::warning title=Full rebuild over budget::The median of ${FULL_REBUILD_OPERATION} is ${median}, over its budget of ${budget} (technical design section 14). The owner decides with an ADR that raises the budget or schedules speed work (ADR 0054).`
}

/**
 * Runs the step of full-rebuild as GitHub runs a `bash` step, in a new folder, with the step's `env`, the runner of
 * macos-15 and an empty job summary, and the cargo stub first on `PATH` printing `stdout` and `stderr` and exiting
 * with `status`. Resolves to the result, the argument lists cargo received and the summary the step wrote.
 */
async function runFullRebuild(context, { stdout = '', stderr = '', status = 0 }) {
  const steps = jobSteps(await workflowJob('platforms.yml', FULL_REBUILD))
  const step = steps[namedStep(steps, FULL_REBUILD_STEP)]
  const directory = await temporaryDirectory(context)
  const workspace = join(directory, 'workspace')
  const stubDirectory = join(directory, 'stub')
  const callsPath = join(directory, 'calls')
  const stdoutPath = join(directory, 'stdout')
  const stderrPath = join(directory, 'stderr')
  const summaryPath = join(directory, 'summary')
  await Promise.all([mkdir(workspace), mkdir(stubDirectory)])
  await Promise.all([
    writeFile(join(stubDirectory, 'cargo'), CARGO_STUB),
    writeFile(callsPath, ''),
    writeFile(stdoutPath, stdout),
    writeFile(stderrPath, stderr),
    writeFile(summaryPath, ''),
  ])
  await chmod(join(stubDirectory, 'cargo'), 0o755)
  const environment = {
    ...process.env,
    ...stepMapping(step, 'env'),
    PATH: `${stubDirectory}:${process.env.PATH}`,
    RUNNER_OS: 'macOS',
    RUNNER_ARCH: 'ARM64',
    GITHUB_STEP_SUMMARY: summaryPath,
    CARGO_CALLS: callsPath,
    CARGO_STDOUT: stdoutPath,
    CARGO_STDERR: stderrPath,
    CARGO_STATUS: String(status),
  }
  const result = await runStep(context, step, workspace, environment, { pipefail: true })
  return {
    ...result,
    calls: await recordedCalls(callsPath),
    summary: await readFile(summaryPath, 'utf8'),
  }
}

/** Asserts that cargo ran the row once, as the step runs it, and that the step printed what cargo printed first. */
function assertBenchmarkRan(result, stdout) {
  assert.deepEqual(result.calls, [
    ['bench', '-p', 'partsmith-recompute', '--bench', 'budgets', '--', FULL_REBUILD_OPERATION],
  ])
  assert.ok(result.stdout.startsWith(stdout), result.stdout)
}

test('105: a median within the budget goes to the job summary with no warning', async (context) => {
  const stdout = `${fullRebuildRow('812.345ms', '1.000s')}\nEvery measured operation is within its budget.\n`

  const result = await runFullRebuild(context, { stdout })

  assertBenchmarkRan(result, stdout)
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`)
  assert.doesNotMatch(result.stdout, /^::/m)
  assert.equal(result.summary, fullRebuildSummary('812.345ms', '1.000s', WITHIN_BUDGET))
})

test('105: a median over the budget goes to the job summary with a warning, and the step passes', async (context) => {
  const stdout = `${fullRebuildRow('1.780s', '1.000s')}\nOver budget: ${FULL_REBUILD_OPERATION} (1.780s > 1.000s)\n`

  const result = await runFullRebuild(context, { stdout, status: 1 })

  assertBenchmarkRan(result, stdout)
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`)
  assert.equal(lastLine(result.stdout), overBudgetWarning('1.780s', '1.000s'))
  assert.equal(result.summary, fullRebuildSummary('1.780s', '1.000s', OVER_BUDGET))
})

for (const [what, run] of [
  [
    'bench50 is missing',
    {
      stdout: `examples/bench/fifty-features/model.ptsm does not exist yet (engine.md open issue 4).\nNot measured: ${FULL_REBUILD_OPERATION}\n`,
      status: 1,
    },
  ],
  ['the build fails', { stderr: 'error: could not compile partsmith-recompute\n', status: 101 }],
]) {
  test(`105: the step fails with Full rebuild not measured and writes no summary when ${what}`, async (context) => {
    const result = await runFullRebuild(context, run)

    assertBenchmarkRan(result, run.stdout ?? '')
    assert.equal(result.status, 1, `${result.stdout}${result.stderr}`)
    assert.equal(lastLine(result.stdout), NOT_MEASURED)
    assert.equal(result.summary, '')
  })
}

test('118: ci.yml and release.yml each have a sources job on ubuntu-24.04, with its name, its needs and its steps in order', async () => {
  for (const { file, job, steps } of await sourcesJobs()) {
    assert.equal(unquoted(jobValue(job, 'name') ?? ''), SOURCE_ARTIFACTS[file].name, file)
    assert.equal(jobValue(job, 'runs-on'), 'ubuntu-24.04', file)
    assert.equal(timeoutMinutes(job), 60, file)
    for (const key of ['strategy', 'defaults', 'env', 'permissions']) {
      assert.equal(jobValue(job, key), undefined, `${file}: the sources job sets ${key}`)
    }
    if (file === 'release.yml') {
      assert.deepEqual(jobNeeds(job), ['verify'], file)
      assert.equal(jobValue(job, 'if'), RELEASE_SOURCES_CONDITION, file)
    } else {
      assert.deepEqual(jobNeeds(job), [], file)
      assert.equal(jobValue(job, 'if'), undefined, file)
    }

    // The cache step comes before the archive step in ci.yml. release.yml has none.
    const cached = file !== UNCACHED_ARCHIVE_FILE
    const archive = cached ? 2 : 1
    const [toolchain, node, writer, partsmith, dependencies, upload] = steps.slice(archive + 1)

    assert.equal(steps.length, archive + 7, `${file}: the sources job has ${steps.length} steps`)
    assert.equal(stepValue(steps[0], 'uses'), pinned('actions/checkout'), file)
    if (cached) {
      assert.equal(stepValue(steps[1], 'id'), 'occt-source', file)
    }
    assert.equal(stepValue(steps[archive], 'run'), ARCHIVE_STEP_RUN, file)
    assert.equal(stepValue(toolchain, 'run'), TOOLCHAIN_STEP, file)
    assert.equal(stepValue(node, 'uses'), pinned('actions/setup-node'), file)
    assert.deepEqual(stepMapping(node, 'with'), { 'node-version': '22' }, file)
    assert.equal(stepValue(writer, 'name'), ARCHIVE_WRITER, file)
    assert.equal(stepValue(partsmith, 'name'), PARTSMITH_WRITER, file)
    assert.equal(stepValue(partsmith, 'run'), PARTSMITH_WRITER_RUN, file)
    for (const key of ['env', 'shell', 'working-directory']) {
      assert.equal(stepValue(partsmith, key), undefined, `${file}: ${PARTSMITH_WRITER} sets ${key}`)
    }
    assert.equal(stepValue(dependencies, 'name'), DEPENDENCY_WRITER, file)
    assert.equal(stepValue(upload, 'uses'), pinned('actions/upload-artifact'), file)
    assert.equal(stepMapping(upload, 'with')?.name, SOURCE_ARTIFACTS[file].artifact, file)
  }
})

/** The lines of `step` without its `run` and the lines of a block scalar below it. */
function withoutRun(step) {
  return step.filter((line) => !line.startsWith('run:') && !line.startsWith('  '))
}

/** The lines of `step` without the `name` entry of its `with`. */
function withoutArtifactName(step) {
  return step.filter((line) => !line.startsWith('  name:'))
}

/** The steps of a sources job without the cache step and without the `if` of the archive step, as release.yml has them. */
function withoutArchiveCache(steps) {
  return steps
    .filter((step) => stepValue(step, 'id') !== 'occt-source')
    .map((step) =>
      stepValue(step, 'run') === ARCHIVE_STEP_RUN
        ? step.filter((line) => !line.startsWith('if:'))
        : step,
    )
}

test("118: release.yml's sources job differs from ci.yml's only in its archive cache, scope and artifact", async () => {
  const [ci, release] = await sourcesJobs()
  const expected = withoutArchiveCache(ci.steps)
  const writer = namedStep(expected, DEPENDENCY_WRITER)
  const upload = expected.length - 1

  assert.equal(release.steps.length, expected.length)
  for (const [i, step] of release.steps.entries()) {
    if (i === writer) {
      assert.deepEqual(withoutRun(step), withoutRun(expected[i]), `release.yml step ${i}`)
    } else if (i === upload) {
      assert.deepEqual(
        withoutArtifactName(step),
        withoutArtifactName(expected[i]),
        `release.yml step ${i}`,
      )
    } else {
      assert.deepEqual(step, expected[i], `release.yml step ${i}`)
    }
  }
})

/** Whether a job's `if` lets it run after a failed job it needs, unless it asks for a passing sources job. */
function runsAfterFailure(condition) {
  return (
    condition !== undefined &&
    /\b(?:always|cancelled|failure|success)\(\)/.test(condition) &&
    !condition.includes("needs.sources.result == 'success'")
  )
}

/**
 * The jobs of `jobs` from `id` up to sources through their `needs`, `id` included and sources left out: every job on a
 * way from `id` to sources. Empty when `id` does not reach sources.
 */
function waysToSources(jobs, id, seen = new Set()) {
  if (id === SOURCES_JOB) {
    return [SOURCES_JOB]
  }
  if (seen.has(id)) {
    return []
  }
  seen.add(id)
  const job = jobs.find((listed) => listed.id === id)
  const above = (job === undefined ? [] : jobNeeds(job)).flatMap((needed) =>
    waysToSources(jobs, needed, seen),
  )
  return above.length === 0 ? [] : [id, ...above.filter((listed) => listed !== SOURCES_JOB)]
}

/** The jobs of `jobs` other than sources that have a step that uses actions/upload-artifact, in order. */
function uploadingJobs(jobs) {
  return jobs.filter(
    (job) =>
      job.id !== SOURCES_JOB &&
      jobSteps(job).some((step) => usesAction(step, 'actions/upload-artifact')),
  )
}

test('118: every job that uploads an artifact reaches sources through its needs, and none runs after sources failed', async () => {
  for (const [file, text] of await workflowTexts(SOURCE_RULE_FILES)) {
    const jobs = workflowJobs(text)
    const uploaders = uploadingJobs(jobs)

    // platforms.yml uploads nothing since the move. A job that uploads there finds no sources job and fails the walk.
    assert.ok(
      uploaders.length > 0 || !Object.hasOwn(SOURCE_ARTIFACTS, file),
      `${file} has no job that uploads an artifact`,
    )
    for (const uploader of uploaders) {
      const way = waysToSources(jobs, uploader.id)
      assert.ok(
        way.length > 0,
        `${file}: ${uploader.id} does not reach ${SOURCES_JOB} through its needs`,
      )
      for (const id of new Set(way)) {
        const condition = jobValue(findJob(jobs, file, id), 'if')
        assert.ok(
          !runsAfterFailure(condition),
          `${file}: ${id} runs after a failed job it needs with if: ${condition}`,
        )
      }
    }
  }
})

test("118: today's jobs that upload an artifact need what section 2.13 lists", async () => {
  const expected = {
    'ci.yml': {
      'core-wasm': ['occt-native', 'occt-wasm', 'sources'],
      web: ['core-wasm', 'sources'],
      e2e: ['core-wasm'],
      'e2e-engines': ['core-wasm'],
    },
    'release.yml': { bundle: ['verify', 'sources'] },
    'platforms.yml': {},
  }
  for (const [file, text] of await workflowTexts(Object.keys(expected))) {
    const uploaders = uploadingJobs(workflowJobs(text))

    assert.deepEqual(
      Object.fromEntries(uploaders.map((job) => [job.id, jobNeeds(job)])),
      expected[file],
      file,
    )
  }
  const release = workflowJobs(await workflowText('release.yml'))
  assert.equal(jobValue(findJob(release, 'release.yml', 'bundle'), 'if'), BUNDLE_CONDITION)
  assert.deepEqual(jobNeeds(findJob(release, 'release.yml', 'draft')), ['bundle', 'sources'])
})

test('118: platforms.yml has no sources job and no step that uploads an artifact', async () => {
  assert.ok(
    !(await workflowJobIds('platforms.yml')).includes(SOURCES_JOB),
    `platforms.yml has a job ${SOURCES_JOB}`,
  )
  assert.deepEqual(actionSteps(await workflowText('platforms.yml'), 'actions/upload-artifact'), [])
})

/** A job as `workflowJobs` reads it, with only the `needs` and `if` lines given. */
function literalJob(id, needs, condition) {
  return {
    id,
    lines: [
      ...(needs === undefined ? [] : [`    needs: ${needs}`]),
      ...(condition === undefined ? [] : [`    if: ${condition}`]),
    ],
  }
}

test('118: a job that uploads an artifact without reaching sources, or after a failed sources job, fails the walk', () => {
  const jobs = [
    literalJob(SOURCES_JOB),
    literalJob('build'),
    literalJob('core', `[build, ${SOURCES_JOB}]`),
    literalJob('web', 'core'),
    literalJob('loose', 'build'),
    literalJob('always', 'core', '${{ always() }}'),
    literalJob('guarded', SOURCES_JOB, "${{ !cancelled() && needs.sources.result == 'success' }}"),
  ]

  assert.deepEqual(waysToSources(jobs, 'web'), ['web', 'core'])
  assert.deepEqual(waysToSources(jobs, 'loose'), [])
  assert.ok(runsAfterFailure(jobValue(findJob(jobs, 'test', 'always'), 'if')))
  assert.ok(!runsAfterFailure(jobValue(findJob(jobs, 'test', 'guarded'), 'if')))
  assert.ok(!runsAfterFailure(undefined))
})

test('119: web downloads sources-web into ui/dist after the source stamp and before the distribution check', async () => {
  const steps = await ciJobSteps('web')
  const stamp = namedStep(steps, 'Write the source stamp')
  const download = stepIndex(steps, 'that downloads sources-web into ui/dist', (step) =>
    downloadsArtifact(step, 'sources-web', 'ui/dist'),
  )
  const check = commandStep(steps, 'pnpm check:dist')

  assert.ok(
    stamp < download && download < check,
    'the download of sources-web is not between the source stamp and pnpm check:dist',
  )
})

test('119: web makes none of the archives itself and restores no cache', async () => {
  const steps = await ciJobSteps('web')

  for (const action of ['actions/cache', 'actions/cache/restore']) {
    assert.ok(!steps.some((step) => usesAction(step, action)), `web uses ${action}`)
  }
  for (const name of [ARCHIVE_WRITER, DEPENDENCY_WRITER]) {
    assert.ok(!steps.some((step) => stepValue(step, 'name') === name), `web has the step ${name}`)
  }
  for (const word of ['rustup', 'dependency-source-archive.mjs']) {
    assert.ok(
      !steps.some((step) => stepValue(step, 'run')?.includes(word)),
      `a step of web runs ${word}`,
    )
  }
})

test("119: the Partsmith writer of the sources job writes the archive of the checkout's commit into sources/", async (context) => {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const repository = join(directory, 'repository')
  await mkdir(join(repository, 'scripts'), { recursive: true })
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFile(join(repository, 'README.md'), '# Partsmith\n')
  await copyFile(join(repositoryRoot, PARTSMITH_SCRIPT), join(repository, PARTSMITH_SCRIPT))
  await chmod(join(repository, PARTSMITH_SCRIPT), 0o755)
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Partsmith')
  const commit = git(environment, repository, 'rev-parse', 'HEAD')
  // The checkout of a CI job is a clone at depth 1.
  const clone = join(directory, 'clone')
  git(
    environment,
    directory,
    'clone',
    '--quiet',
    '--depth',
    '1',
    pathToFileURL(repository).href,
    clone,
  )
  await mkdir(join(clone, SOURCES_FOLDER))
  const steps = jobSteps(await workflowJob('ci.yml', SOURCES_JOB))
  const expected = join(directory, 'expected.tar.gz')
  writePartsmithArchive(environment, repository, commit, expected)

  const result = await runStep(
    context,
    steps[namedStep(steps, PARTSMITH_WRITER)],
    clone,
    environment,
  )

  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(await readdir(join(clone, SOURCES_FOLDER)), [PARTSMITH_ARCHIVE])
  assert.deepEqual(
    await readFile(join(clone, SOURCES_FOLDER, PARTSMITH_ARCHIVE)),
    await readFile(expected),
  )
})

test("120: the deploy job checks Partsmith's archive after the dependency archives and before Wrangler, with the run of section 2.4", async () => {
  const steps = await ciJobSteps('deploy')
  const dependencies = namedStep(steps, DEPENDENCY_CHECK)
  const check = namedStep(steps, PARTSMITH_CHECK)

  assert.equal(stepValue(steps[check], 'if'), "steps.secrets.outputs.available == 'true'")
  assert.ok(
    dependencies < check && check < idStep(steps, 'pages'),
    `${PARTSMITH_CHECK} is not between ${DEPENDENCY_CHECK} and Wrangler`,
  )
  assert.equal(stepValue(steps[check], 'run'), PARTSMITH_CHECK_RUN)
})

/** The Partsmith-source error of section 4 for the run's commit `commit`. */
function partsmithErrorMessage(commit) {
  return `::error title=Partsmith source missing::ui/dist/${PARTSMITH_ARCHIVE} is missing or does not hold the source of the commit ${commit}, so nothing was deployed. The sources job writes it, and the web job puts it into the build (docs/spec/delivery.md section 2.13).`
}

/**
 * Makes the fixture of test 120 in a new folder: a repository with two commits, the second adding big.bin, whose tar
 * is larger than a pipe holds, and the archives of both commits. Resolves to the folder, the Git environment, the
 * repository, both commits and both archives' paths.
 */
async function partsmithCheckFixture(context) {
  const directory = await temporaryDirectory(context)
  const environment = await isolatedGitEnvironment(directory)
  const repository = join(directory, 'repository')
  await mkdir(repository)
  git(environment, repository, 'init', '--initial-branch=main')
  await writeFile(join(repository, 'README.md'), '# Partsmith\n')
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'First')
  const first = git(environment, repository, 'rev-parse', 'HEAD')
  await writeFile(join(repository, 'big.bin'), Buffer.alloc(PIPE_BREAKING_SIZE))
  git(environment, repository, 'add', '--all')
  git(environment, repository, 'commit', '--message', 'Second')
  const second = git(environment, repository, 'rev-parse', 'HEAD')
  const firstArchive = join(directory, 'first.tar.gz')
  const secondArchive = join(directory, 'second.tar.gz')
  writePartsmithArchive(environment, repository, first, firstArchive)
  writePartsmithArchive(environment, repository, second, secondArchive)
  return { directory, environment, repository, first, second, firstArchive, secondArchive }
}

/**
 * Runs the deploy job's Partsmith check in a new folder that holds a copy of the check script, with `GITHUB_SHA` the
 * fixture's second commit, after writing `files` into `ui/dist/` (name to bytes; no `ui/dist` when `files` is
 * undefined), in the fixture's Git environment. With `ignorePipe`, the shell that starts the step ignores SIGPIPE.
 */
function runPartsmithCheck(context, fixture, files, { ignorePipe = false } = {}) {
  return runDeployCheck(context, PARTSMITH_CHECK, {
    files,
    environment: { ...fixture.environment, GITHUB_SHA: fixture.second },
    ignorePipe,
  })
}

/** Asserts that the Partsmith check fails with exactly the Partsmith-source error, as `runPartsmithCheck` takes `files`. */
async function assertPartsmithRejected(context, fixture, files) {
  const result = await runPartsmithCheck(context, fixture, files)

  assert.equal(result.status, 1, result.stderr)
  assert.equal(result.stdout, `${partsmithErrorMessage(fixture.second)}\n`)
  assert.equal(result.stderr, '')
}

test("120: the Partsmith check passes silently on the run's commit's archive, with SIGPIPE at its default and ignored", async (context) => {
  const fixture = await partsmithCheckFixture(context)
  const files = { [PARTSMITH_ARCHIVE]: await readFile(fixture.secondArchive) }

  // The fixture breaks the pipe: git exits after the first header while gzip is still writing.
  const brokenPipe = runShell(
    fixture.environment,
    `trap '' PIPE; gzip -dc "$1" | git get-tar-commit-id`,
    fixture.secondArchive,
  )
  assert.equal(brokenPipe.stdout, `${fixture.second}\n`)
  assert.match(brokenPipe.stderr, /^gzip: stdout: Broken pipe$/m)

  const runs = [false, true]
  const results = await Promise.all(
    runs.map((ignorePipe) => runPartsmithCheck(context, fixture, files, { ignorePipe })),
  )

  for (const [i, result] of results.entries()) {
    const label = `SIGPIPE ${runs[i] ? 'ignored' : 'at its default'}`
    assert.equal(result.status, 0, `${label}: ${result.stdout}${result.stderr}`)
    assert.equal(result.stdout, '', label)
    assert.equal(result.stderr, '', label)
  }
})

test('120: the Partsmith check fails with the Partsmith-source error when the archive is missing', async (context) => {
  await assertPartsmithRejected(context, await partsmithCheckFixture(context), {})
})

test('120: the Partsmith check fails with the Partsmith-source error when ui/dist is missing', async (context) => {
  await assertPartsmithRejected(context, await partsmithCheckFixture(context), undefined)
})

test('120: the Partsmith check fails with the Partsmith-source error when the archive holds another commit', async (context) => {
  const fixture = await partsmithCheckFixture(context)

  await assertPartsmithRejected(context, fixture, {
    [PARTSMITH_ARCHIVE]: await readFile(fixture.firstArchive),
  })
})

test('120: the Partsmith check fails with the Partsmith-source error when the archive is a tar without gzip', async (context) => {
  const fixture = await partsmithCheckFixture(context)

  await assertPartsmithRejected(context, fixture, {
    [PARTSMITH_ARCHIVE]: gunzipSync(await readFile(fixture.secondArchive)),
  })
})

test('120: the Partsmith check fails with the Partsmith-source error when the archive is empty', async (context) => {
  await assertPartsmithRejected(context, await partsmithCheckFixture(context), {
    [PARTSMITH_ARCHIVE]: '',
  })
})

test("120: the Partsmith check fails with the Partsmith-source error when the archive is a whole gzip file of the commit's tree, whose tar names no commit", async (context) => {
  const fixture = await partsmithCheckFixture(context)
  const treeArchive = join(fixture.directory, 'tree.tar.gz')
  writePartsmithArchive(
    fixture.environment,
    fixture.repository,
    `${fixture.second}^{tree}`,
    treeArchive,
  )

  assert.equal(runShell(fixture.environment, 'gzip -t "$1"', treeArchive).status, 0)
  assert.equal(
    runShell(fixture.environment, 'gzip -dc "$1" | git get-tar-commit-id', treeArchive).stdout,
    '',
  )
  await assertPartsmithRejected(context, fixture, {
    [PARTSMITH_ARCHIVE]: await readFile(treeArchive),
  })
})

test("120: the Partsmith check fails with the Partsmith-source error when the archive lacks gzip's checksum and length", async (context) => {
  const fixture = await partsmithCheckFixture(context)
  const cutOff = join(fixture.directory, 'cut-off.tar.gz')
  await writeFile(cutOff, (await readFile(fixture.secondArchive)).subarray(0, -8))

  // Its first header still names the commit. Only gzip -t tells it apart.
  assert.equal(
    runShell(fixture.environment, 'gzip -dc "$1" 2>/dev/null | git get-tar-commit-id', cutOff)
      .stdout,
    `${fixture.second}\n`,
  )
  await assertPartsmithRejected(context, fixture, { [PARTSMITH_ARCHIVE]: await readFile(cutOff) })
})

test('122: the check script exits 2 with its usage line for other than two arguments', () => {
  for (const scriptArguments of [[], ['archive.tar.gz'], ['archive.tar.gz', 'commit', 'extra']]) {
    const result = spawnSync('bash', [join(repositoryRoot, CHECK_SCRIPT), ...scriptArguments], {
      encoding: 'utf8',
    })

    assert.equal(result.status, 2, `${scriptArguments.length} arguments`)
    assert.equal(result.stdout, '')
    assert.equal(result.stderr, `${CHECK_USAGE}\n`)
  }
})

test('122: no step reads the commit of a source archive itself, and each archive check runs the check script once', async () => {
  for (const [file, text] of await workflowTexts(SOURCE_RULE_FILES)) {
    for (const job of workflowJobs(text)) {
      for (const step of jobSteps(job)) {
        assert.ok(
          !(stepValue(step, 'run') ?? '').includes('get-tar-commit-id'),
          `${file}: a step of ${job.id} runs git get-tar-commit-id`,
        )
      }
    }
  }
  const checks = [
    ...Object.keys(SOURCE_ARTIFACTS).map((file) => [file, SOURCES_JOB, ARCHIVE_WRITER]),
    ['ci.yml', 'deploy', ARCHIVE_CHECK],
    ['ci.yml', 'deploy', PARTSMITH_CHECK],
  ]
  const runs = await Promise.all(
    checks.map(async ([file, id, name]) => {
      const steps = jobSteps(await workflowJob(file, id))
      return stepValue(steps[namedStep(steps, name)], 'run')
    }),
  )

  for (const [i, run] of runs.entries()) {
    const [file, , name] = checks[i]
    assert.equal(run.split(CHECK_SCRIPT).length - 1, 1, `${file}: ${name} runs ${CHECK_SCRIPT}`)
  }
})

/** The lines of a job without its comment lines and blank lines, in order. */
function jobContentLines(job) {
  return job.lines.filter((line) => !isBlankOrComment(line))
}

/**
 * The lines in which the job `copy` differs from the job `original`, compared one by one in order without comment
 * lines and blank lines, as `[original line, copy line]` pairs; a line that one job has and the other lacks pairs with
 * undefined.
 */
function differingLines(original, copy) {
  const originalLines = jobContentLines(original)
  const copyLines = jobContentLines(copy)
  const pairs = []
  for (let i = 0; i < Math.max(originalLines.length, copyLines.length); i++) {
    if (originalLines[i] !== copyLines[i]) {
      pairs.push([originalLines[i], copyLines[i]])
    }
  }
  return pairs
}

test('128: the moved jobs come right after desktop and right before deploy, named as the required checks', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))
  const ids = jobs.map((job) => job.id)
  const desktop = ids.indexOf(findJob(jobs, 'ci.yml', 'desktop').id)
  const moved = Object.keys(MOVED_JOBS)

  assert.deepEqual(ids.slice(desktop + 1, desktop + moved.length + 2), [...moved, 'deploy'])
  for (const [id, name] of Object.entries(MOVED_JOBS)) {
    assert.equal(unquoted(jobValue(findJob(jobs, 'ci.yml', id), 'name') ?? ''), name, id)
  }
})

test('128: no line of the moved jobs reads a secret, so they run on a pull request from a fork', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))

  for (const id of Object.keys(MOVED_JOBS)) {
    const line = findJob(jobs, 'ci.yml', id).lines.find((jobLine) => jobLine.includes('secrets.'))
    assert.equal(line, undefined, `${id} reads a secret in: ${line}`)
  }
})

test('128: ci.yml still cancels a run of its ref that a newer one supersedes (choice 1)', async () => {
  const concurrency = topLevelLines(await workflowText('ci.yml'), 'concurrency')

  assert.deepEqual(mappingEntries(concurrency), {
    group: 'ci-${{ github.ref }}',
    'cancel-in-progress': 'true',
  })
})

test('128: in ci.yml, occt-platforms needs nothing and rust-platforms needs it alone', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))

  assert.deepEqual(jobNeeds(findJob(jobs, 'ci.yml', 'occt-platforms')), [])
  assert.deepEqual(jobNeeds(findJob(jobs, 'ci.yml', 'rust-platforms')), ['occt-platforms'])
})

test('128: no step of the moved jobs is allowed to fail', async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))

  for (const id of Object.keys(MOVED_JOBS)) {
    for (const step of jobSteps(findJob(jobs, 'ci.yml', id))) {
      assert.equal(
        stepValue(step, 'continue-on-error'),
        undefined,
        `a step of ${id} sets continue-on-error`,
      )
    }
  }
})

test("128: platforms.yml's occt-platforms is ci.yml's but for its own name and its one macOS row (choice 3)", async () => {
  const [ci, platforms] = await Promise.all([
    workflowJob('ci.yml', 'occt-platforms'),
    workflowJob('platforms.yml', 'occt-platforms'),
  ])

  assert.deepEqual(differingLines(ci, platforms), [
    [`    name: ${MOVED_JOBS['occt-platforms']}`, COPY_NAME_LINE],
    ['        runner: [windows-2025, macos-15]', COPY_MATRIX_LINE],
  ])
})

test('128: platforms.yml dropped the moved jobs and the web jobs that only e2e-engines needed', async () => {
  const ids = await workflowJobIds('platforms.yml')

  for (const id of DROPPED_PLATFORMS_JOBS) {
    assert.ok(!ids.includes(id), `platforms.yml has a job ${id}`)
  }
})

test('128: full-rebuild is a job of platforms.yml alone', async () => {
  for (const [file, text] of await workflowTexts(['ci.yml', 'nightly.yml', 'release.yml'])) {
    const ids = workflowJobs(text).map((job) => job.id)
    assert.ok(!ids.includes(FULL_REBUILD), `${file} has a job ${FULL_REBUILD}`)
  }
})

test('128: no job of platforms.yml reports a check under the name of a job of ci.yml', async () => {
  const [ci, platforms] = await Promise.all(['ci.yml', 'platforms.yml'].map(workflowText))
  const ciNames = new Set(workflowJobs(ci).map((job) => jobValue(job, 'name')))

  for (const job of workflowJobs(platforms)) {
    const name = jobValue(job, 'name')
    assert.ok(!ciNames.has(name), `platforms.yml's ${job.id} is named ${name}, as a job of ci.yml`)
  }
})

test('128: the copy comparison leaves out comment and blank lines and pairs each line that differs', () => {
  const original = {
    id: 'job',
    lines: [
      '    name: A',
      '    # Why it runs.',
      '    runs-on: x',
      '',
      '    steps:',
      '      - run: one',
    ],
  }
  const copy = {
    id: 'job',
    lines: [
      '    name: B',
      '    runs-on: x',
      '    # Another reason.',
      '    steps:',
      '      - run: one',
      '      - run: two',
    ],
  }

  assert.deepEqual(differingLines(original, copy), [
    ['    name: A', '    name: B'],
    [undefined, '      - run: two'],
  ])
  assert.deepEqual(differingLines(original, original), [])
})

/**
 * The keys of a step and their values, as an object: a mapping such as `with` as the object of its entries, a block
 * scalar as its text, and any other value as written. Comment lines and the lines of a block are no keys.
 */
function stepEntries(step) {
  const entries = {}
  for (const line of step) {
    const key = /^([\w-]+):/.exec(line)?.[1]
    if (key !== undefined) {
      const value = stepValue(step, key)
      entries[key] = value === '' ? stepMapping(step, key) : value
    }
  }
  return entries
}

/**
 * The steps of `steps` after the package install and before the step that runs `tests`, where section 3.2 puts the
 * apt cache; fails the test when either step is missing or the tests come first.
 */
function stepsAroundInstall(steps, tests) {
  const install = commandStep(steps, PACKAGE_INSTALL)
  const run = commandStep(steps, tests)
  assert.ok(install < run, `the tests (${tests}) run before ${PACKAGE_INSTALL}`)
  return steps.slice(install + 1, run)
}

/**
 * The steps `steps` as a workflow writes them, without their indentation: `- ` before each step's first line and two
 * spaces before its other lines, with comment lines left out.
 */
function writtenSteps(steps) {
  return steps.flatMap((step) =>
    step
      .filter((line) => !isBlankOrComment(line))
      .map((line, i) => (i === 0 ? `- ${line}` : `  ${line}`)),
  )
}

/** The lines of the fenced code block of a Markdown `text` whose first line is `firstLine`; undefined when there is none. */
function fencedBlock(text, firstLine) {
  const lines = text.split('\n')
  const start = lines.findIndex((line, i) => line === firstLine && lines[i - 1]?.startsWith('```'))
  return start === -1 ? undefined : lines.slice(start, lines.indexOf('```', start))
}

/**
 * The version at which the importer `.` of a pnpm lock file's `text` locks the package `name`, among its dependencies
 * and dev dependencies, without the peers that pnpm writes after it in parentheses; undefined when it locks none.
 */
function lockedVersion(text, name) {
  const importer = blockBelow(text.split('\n'), '  .:', 4) ?? []
  const keys = new Set([`  ${name}:`, `  '${name}':`])
  const groups = new Set(['dependencies:', 'devDependencies:'])
  let group
  for (const [i, line] of importer.entries()) {
    if (!line.startsWith(' ')) {
      group = line
    } else if (groups.has(group) && keys.has(line)) {
      const version = indentedLines(importer, i)
        .map((entry) => /^ {2}version: (.*)$/.exec(entry)?.[1])
        .find((entry) => entry !== undefined)
      return version?.replace(/\(.*$/, '')
    }
  }
  return undefined
}

for (const [id, { browser, tests }] of Object.entries(E2E_JOBS)) {
  test(`130: ${id} runs the six steps of section 3.2 for ${browser} between its package install and its tests`, async () => {
    const job = await workflowJob('ci.yml', id)
    const runner = jobValue(job, 'runs-on')

    assert.equal(runner, E2E_RUNNER)
    assert.deepEqual(
      stepsAroundInstall(jobSteps(job), tests).map(stepEntries),
      aptCacheSteps(browser, runner),
    )
  })
}

test("130: e2e-engines' six steps are the YAML of section 3.2, line for line", async () => {
  const [job, spec] = await Promise.all([
    workflowJob('ci.yml', 'e2e-engines'),
    repositoryText('docs/spec/delivery.md'),
  ])
  const block = fencedBlock(spec, APT_STEPS_FIRST_LINE)

  assert.ok(block, `section 3.2 has no YAML block that starts with ${APT_STEPS_FIRST_LINE}`)
  assert.deepEqual(
    writtenSteps(stepsAroundInstall(jobSteps(job), E2E_JOBS['e2e-engines'].tests)),
    block.filter((line) => !isBlankOrComment(line)),
  )
})

test("130: e2e's six steps are e2e-engines' with chromium for the matrix's browser, comment and blank lines aside", async () => {
  const jobs = workflowJobs(await workflowText('ci.yml'))
  const [engines, chromium] = ['e2e-engines', 'e2e'].map((id) =>
    writtenSteps(stepsAroundInstall(jobSteps(findJob(jobs, 'ci.yml', id)), E2E_JOBS[id].tests)),
  )

  assert.deepEqual(
    chromium,
    engines.map((line) => line.replaceAll(E2E_JOBS['e2e-engines'].browser, E2E_JOBS.e2e.browser)),
  )
})

test('130: no other job of the workflows installs a browser or names the apt archives', async () => {
  for (const [file, text] of await workflowTexts(WORKFLOW_FILES)) {
    for (const job of workflowJobs(text)) {
      if (file === 'ci.yml' && Object.hasOwn(E2E_JOBS, job.id)) {
        continue
      }
      for (const step of jobSteps(job)) {
        const run = stepValue(step, 'run') ?? ''
        assert.ok(!run.includes('playwright install'), `${file}: ${job.id} runs ${run}`)
      }
      const line = job.lines.find((jobLine) => jobLine.includes('apt-archives'))
      assert.equal(line, undefined, `${file}: ${job.id} names the apt archives in: ${line}`)
    }
  }
})

test('130: the version step writes the Playwright version that the lock file and package.json pin', async (context) => {
  const [lock, manifest, job] = await Promise.all([
    repositoryText('ui/pnpm-lock.yaml'),
    repositoryText('ui/package.json'),
    workflowJob('ci.yml', 'e2e-engines'),
  ])
  const steps = jobSteps(job)
  const version = lockedVersion(lock, '@playwright/test')
  const directory = await temporaryDirectory(context)
  const scriptPath = join(directory, 'playwright-version.sh')
  const outputPath = join(directory, 'github-output')
  await writeFile(scriptPath, stepValue(steps[idStep(steps, 'playwright')], 'run'))
  await writeFile(outputPath, '')

  const result = spawnSync('bash', ['-e', scriptPath], {
    cwd: join(repositoryRoot, 'ui'),
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${dirname(process.execPath)}:${process.env.PATH}`,
      GITHUB_OUTPUT: outputPath,
    },
  })

  assert.equal(result.status, 0, result.stderr)
  assert.match(version ?? '', /^\d+\.\d+\.\d+$/)
  assert.equal(JSON.parse(manifest).devDependencies['@playwright/test'], version)
  assert.equal(await readFile(outputPath, 'utf8'), `version=${version}\n`)
})

test("130: the copy out of apt's cache is a script that bash reads", async (context) => {
  const steps = jobSteps(await workflowJob('ci.yml', 'e2e-engines'))
  const copy = steps[namedStep(steps, "Copy the apt archives out of apt's cache")]
  const scriptPath = join(await temporaryDirectory(context), 'copy-out.sh')
  await writeFile(scriptPath, stepValue(copy, 'run'))

  const result = spawnSync('bash', ['-n', scriptPath], { encoding: 'utf8' })

  assert.equal(result.status, 0, result.stderr)
})

test('130: stepEntries reads scalars, block scalars and mappings, and leaves out comments', () => {
  const step = [
    'name: Save',
    '# Why it saves.',
    'if: a == b',
    'uses: actions/cache/save@v6.1.0',
    'with:',
    '  path: ~/apt-archives',
    '  key: k',
    'run: |',
    '  echo one',
    '  echo two',
  ]

  assert.deepEqual(stepEntries(step), {
    name: 'Save',
    if: 'a == b',
    uses: 'actions/cache/save@v6.1.0',
    with: { path: '~/apt-archives', key: 'k' },
    run: 'echo one\necho two\n',
  })
})

test('130: the steps around the install are those between the package install and the tests', () => {
  const steps = [
    ['uses: actions/checkout@v7.0.1'],
    [`run: ${PACKAGE_INSTALL}`],
    ['name: One', 'run: one'],
    ['name: Two', 'run: two'],
    ['run: pnpm e2e'],
    ['if: failure()'],
  ]

  assert.deepEqual(stepsAroundInstall(steps, 'pnpm e2e'), [steps[2], steps[3]])
  assert.deepEqual(stepsAroundInstall([steps[1], steps[4]], 'pnpm e2e'), [])
  assert.throws(() => stepsAroundInstall([steps[4], steps[1]], 'pnpm e2e'), /run before/)
  assert.throws(() => stepsAroundInstall([steps[1]], 'pnpm e2e'), /no step that runs pnpm e2e/)
})

test('130: writtenSteps writes each step as a workflow does and leaves out its comment lines', () => {
  const steps = [['name: One', '# Why.', 'with:', '  key: k'], ['run: two']]

  assert.deepEqual(writtenSteps(steps), ['- name: One', '  with:', '    key: k', '- run: two'])
})

test('130: fencedBlock finds the code block that starts with a line, and only a code block', () => {
  const text = [
    '- name: Read the Playwright version',
    '',
    '```yaml',
    '- name: Other',
    '```',
    '',
    '```yaml',
    '- name: Read the Playwright version',
    '  id: playwright',
    '```',
  ].join('\n')

  assert.deepEqual(fencedBlock(text, '- name: Read the Playwright version'), [
    '- name: Read the Playwright version',
    '  id: playwright',
  ])
  assert.equal(fencedBlock(text, '- name: Missing'), undefined)
})

test('130: lockedVersion reads a package of the root importer without the peers after its version', () => {
  const lock = [
    "lockfileVersion: '9.0'",
    '',
    'importers:',
    '',
    '  .:',
    '    dependencies:',
    '      lucide-react:',
    '        specifier: 1.55.0',
    '        version: 1.55.0(react@19.3.0)',
    '    devDependencies:',
    "      '@playwright/test':",
    '        specifier: 1.64.0',
    '        version: 1.64.0',
    '',
    'packages:',
  ].join('\n')

  assert.equal(lockedVersion(lock, '@playwright/test'), '1.64.0')
  assert.equal(lockedVersion(lock, 'lucide-react'), '1.55.0')
})

test('130: lockedVersion finds no version in another importer, in packages or in an empty importer', () => {
  const lock = [
    'importers:',
    '',
    '  .:',
    '    devDependencies:',
    '      vite:',
    '        specifier: 8.3.4',
    '        version: 8.3.4',
    '',
    '  docs:',
    '    devDependencies:',
    "      '@playwright/test':",
    '        specifier: 1.63.0',
    '        version: 1.63.0',
    '',
    'packages:',
    '',
    "  '@playwright/test@1.64.0':",
    '    resolution: {integrity: sha512-x}',
  ].join('\n')

  assert.equal(lockedVersion(lock, '@playwright/test'), undefined)
  assert.equal(lockedVersion('importers:\n\n  .: {}\n', 'vite'), undefined)
})
