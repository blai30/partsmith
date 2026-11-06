# .github/workflows

CI, the Cloudflare Pages deploy, the desktop release, the nightly property run and the weekly platforms run. GitHub runs only the `.yml` files here.

Spec: `docs/spec/delivery.md` (the jobs of 2.4 and 2.5, the notices of 2.9, the npm license check of 2.10, the nightly run of 2.11, the platforms run of 2.14, the deploy of 3.3, the release and its checklist of 3.6). Decisions: ADR 0003 (OCCT builds and caches), ADR 0009 (the web core module), ADR 0013 (oxc tooling), ADR 0014 (Tauri and `tauri-cli`), ADR 0017 (the web build and its size gate), ADR 0033 (notices and the npm license check from the lock files), ADR 0034 (OCCT linking and the source route that releases wait for), ADR 0038 (the platforms run outside the gate, and how its jobs move into CI), ADR 0042 (the WebAssembly test steps of `kernel-wasm` and `core-wasm`).

## Interface

- `ci.yml`, on pushes to `main` and pull requests: `occt-native`, `occt-wasm`, `kernel-wasm`, `rust`, `deny`, `ui`, `schema`, `sources`, `core-wasm`, `web`, `e2e`, `python`, `desktop`, `occt-platforms`, `rust-platforms`, `e2e-engines`, then `deploy`. Artifacts, each kept 7 days but `sources-web`, kept 8: `sources-web` (OCCT's source archive `occt-<version>-source.tar.gz`, Partsmith's source archive `partsmith-source.tar.gz` and the dependency source archives in `dependencies/`, which `sources` uploads before any job uploads the core, delivery.md 2.13), `core-wasm` (`ui/public/core/` and `ui/public/THIRD-PARTY-NOTICES.txt`, rooted at `ui/public/`), `web-dist` (`ui/dist/`, with the source stamp `SOURCE.txt` that `web` writes and the files of `sources-web`, delivery.md 2.13), `playwright-report` on an `e2e` failure, and `playwright-report-<browser>` on an `e2e-engines` failure.
- `kernel-wasm` runs three test steps on `wasm32-unknown-emscripten` under Node, in this order: the kernel (`partsmith-occt-sys`), the sketch suite (sketch.md 3.12 and test 255) and the engine's panic containment (`cargo test -p partsmith-engine --lib --target wasm32-unknown-emscripten dispatch::`, protocol.md test 84). `core-wasm` runs `scripts/build-wasm.sh`, then, before the upload, the `web_module::` round trips with `PARTSMITH_REQUIRE_WEB_MODULE=1` and the parity test `cargo test -p partsmith-wasm --test integration web_parity:: -- --ignored` (engine.md test 154: the module against a native session over the engine fixtures and the examples). A `run` that ends in `::` or holds `:: ` is a block scalar. delivery.md test 66 in `ui/scripts/workflows.test.mjs` checks these steps and that each filter still selects tests.
- `ui` ends with `pnpm check:licenses`, which holds the npm packages that ship to the allow list of `deny.toml`. `schema` installs the packages first and ends with `pnpm -C ui check:notices`, which runs after a failure of the schema steps too, as long as the install passed. `desktop` checks that the `.deb` holds `THIRD-PARTY-NOTICES.txt`, equal to the repository's file.
- `python` installs CPython 3.13 and 3.12 (3.12 last, so it is the default `python`), makes its virtual environment with `python3.12`, runs `pytest python/tests`, then `pytest python/tests -m budgets -s` in the step `Measure the Python budgets` (python-api.md section 5: one line per row; a row over its budget warns without failing, a row that cannot be measured fails), then `pytest python/tests -m packaging` (python-api.md test 76: one abi3 wheel, installed and imported with `python3.12` and `python3.13`).
- `occt-platforms` restores or builds OCCT on `windows-2025` and `macos-15` with the steps and key of `release.yml`'s `bundle` job; `rust-platforms`, once both OCCT rows have ended, runs `cargo test --no-fail-fast -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop`, the examples runner and clippy for those three crates there, and on Windows `scripts/check-windows-imports.sh` on `partsmith-desktop.exe` and `partsmith.exe`, which fails when either imports a DLL of the Visual C++ Redistributable or no `api-ms-win-crt` DLL (delivery.md 3.6); `e2e-engines` installs its engine in the step `Install ${{ matrix.browser }} and its system packages` (`pnpm exec playwright install --with-deps`, 10 minutes, with apt's downloaded archives restored before it and, on `main` when it downloaded a package they lacked, saved after it, as in `e2e` and around the Tauri Linux packages of `rust` and `desktop`, delivery.md 3.2) and runs the tests tagged `@engines` in Firefox and WebKit under `xvfb-run` (Firefox runs headed, ui.md 6.8): `xvfb-run --auto-servernum pnpm e2e --project ${{ matrix.browser }}`.
- `deploy` publishes `web-dist` with Wrangler: production on `main`, a preview per pull request, and fails without deploying unless `ui/dist/SOURCE.txt` names the run's commit, `ui/dist/occt-<version>-source.tar.gz` holds the source of the pinned OCCT commit, `ui/dist/dependencies/` holds the parts its `SHA256SUMS` lists and `ui/dist/partsmith-source.tar.gz` holds the source of the run's commit (delivery.md 2.13). It needs the secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` and reads the optional variable `CLOUDFLARE_PAGES_PROJECT` (default `partsmith`); without the secrets it succeeds with a "Deploy skipped" notice.
- `release.yml`, on a `vX.Y.Z` tag: `verify` (the tag equals the version of `partsmith-desktop` and `ui/package.json`, and CI passed for the commit), `sources` (the artifact `sources-desktop`, kept 8 days: OCCT's source archive made from the pinned tag in every run, `partsmith-source.tar.gz` and the parts of the desktop dependency source archives, delivery.md 2.13), `bundle` (Linux, Windows and macOS bundles as `bundle-<runner.os>`, kept 7 days, the Windows app's imports checked with `scripts/check-windows-imports.sh`, delivery.md 3.6), `draft` (a draft GitHub Release with the bundles, `THIRD-PARTY-NOTICES.txt`, the files of `sources-desktop` and `SHA256SUMS`, published by a human). A manual run (`workflow_dispatch`) is a dry run that builds the bundles and uploads them with `sources-desktop`, with no release.
- `nightly.yml`, at 04:23 UTC (cron `23 4 * * *`) and on `workflow_dispatch`: the job `properties` runs the sketch property tests with `PROPTEST_CASES=4096` in release. A scheduled run first asks `gh run list` for the newest finished nightly run on `main`, whatever its conclusion, and when that run tested the head of `main` it stops with a "Nightly skipped" notice before checkout; when `gh` fails, it warns and tests. A manual run always tests.
- `platforms.yml`, on `workflow_dispatch` and on Mondays at 05:41 UTC (cron `41 5 * * 1`), outside the gate: no job of `ci.yml` or `release.yml` waits for it and none is in `deploy.needs`. `occt-platforms` (`OCCT for the full rebuild (macos-15)`) is a copy of `ci.yml`'s `macos-15` row; `full-rebuild` times `bench50` from an empty cache in a release build on `macos-15` and writes its median and its budget (1.8 s, ADR 0057) to the job summary, with a warning and no failure over the budget (technical design section 14, ADR 0054). It uploads no artifact.

## Commands

```sh
# The scripts CI runs and their tests, and the tests of these workflow files (run in ui/)
pnpm test:scripts
pnpm check:licenses
# pnpm notices rewrites both notices files
pnpm check:notices
pnpm build && pnpm check:dist

# The budgets and the packaging test of the python job (packaging needs python3.12 and python3.13 on PATH)
pytest python/tests -m budgets -s
pytest python/tests -m packaging

# The nightly property run
PROPTEST_CASES=4096 cargo test -p partsmith-sketch --release --test integration properties::

# The platforms run on main (release checklist item 2), and its latest runs, which promotion reads
gh workflow run platforms.yml --ref main
gh run list --workflow platforms.yml --branch main --limit 2
gh run list --workflow platforms.yml --branch main --commit <sha> --status success

# The engine subset as far as this machine can run it: in Chromium, and the listings of Firefox and WebKit (run in ui/)
pnpm e2e --project chromium --grep @engines
pnpm e2e --grep E-157

# Release checklist items 4 to 6: the dry run from main, the tag, and the draft's assets checked
gh workflow run release.yml --ref main
git tag vX.Y.Z <commit> && git push origin vX.Y.Z
gh release download vX.Y.Z --dir release-vX.Y.Z && (cd release-vX.Y.Z && sha256sum -c SHA256SUMS)
```

## Rules

- Every `uses:` names an exact release (delivery.md 2.2); tools are pinned where 2.2 says.
- Jobs that need OCCT restore its cache with `fail-on-cache-miss: true`; only `ci.yml`'s `occt-native`, `occt-wasm` and `occt-platforms`, `platforms.yml`'s copy of `occt-platforms`, and `release.yml`'s `bundle` job build it. `occt-platforms` and `bundle` share the OCCT steps and cache key, so a fix one of them needs applies to the other, and either restores what the other saved on `main`.
- Every job that compiles with cargo runs `rustup show active-toolchain || rustup toolchain install` and then `Swatinem/rust-cache`; the `sources` jobs run only `cargo vendor` and `cargo metadata` and take the toolchain step alone. Every job that runs pnpm sets up pnpm from `ui/package.json` and Node 22 with the pnpm cache.
- Every job that uploads an artifact holding OCCT's compiled code needs the job `sources`, directly or through the jobs it needs. A run then never holds that code without its source (delivery.md 2.13, ADR 0034).
- A job that uploads OCCT's compiled code is rerun with `gh run rerun <id> --failed` only within 12 hours of the end of the run's `sources` job. Later, rerun the whole run with `gh run rerun <id>` (delivery.md 2.13).
- No secret other than the two Cloudflare secrets and `GITHUB_TOKEN`. Signing and updater secrets wait for a human decision (delivery.md 1.2).
- Every job sets `timeout-minutes` (delivery.md 2.4, 2.5, 2.11 and 2.14), so a hung job ends long before GitHub's default of 360 minutes: 150 for the jobs that may build OCCT (`occt-native`, `occt-wasm`, `occt-platforms`, `bundle`), 20 for the nightly `properties` and 60 for every other job, a new one included; `rust-platforms`' `crates` step also sets 20, so a hung test ends the step, not the job, and the examples runner and clippy still run; the browser installs of `e2e` and `e2e-engines` set 10, so a slow Ubuntu mirror ends the install at 10 minutes instead of running into the job's 60 (delivery.md open issue 16; the promotion rule below says what such a failure needs); `ui/scripts/workflows.test.mjs` checks every value. The repository is public, so standard GitHub-hosted runners bill nothing (ADR 0034). `nightly.yml` also skips nights when `main` has not changed and needs no OCCT; its cron is the one place to change its cadence (delivery.md 2.11).
- Before each tag, start the release dry run from `main`. It proves the bundles (delivery.md section 6, test 13) and saves the Windows and macOS caches on `main`, the only scope a tag run can restore them from; a tag run's own caches are scoped to its tag, and GitHub drops caches unused for 7 days. CI's `occt-platforms` restores the Windows and macOS OCCT caches on every run, and the weekly platforms run the macOS one, which usually keeps them alive (delivery.md 3.2). The `bundle` job saves its Rust cache only on `main`.
- A job of `platforms.yml` other than `full-rebuild`, which stays there (ADR 0054), or one row of its matrix, moves into `ci.yml` once it has passed in the two latest runs of `platforms.yml` on `main` (`gh run list --workflow platforms.yml --branch main`), with the pull request that delivery.md 2.14 (promotion) describes: the job moves as it is with the jobs it needs, joins `deploy.needs`, and every text and test that names it changes with it. Whenever the `macos-15` row of `occt-platforms` moves, `platforms.yml` keeps a copy of it for `full-rebuild`, and the copy is never promoted. The owner decided on 2026-11-05 that no job moves before it has passed two runs on `main` after wave F. Run 14 (41926898084, on 0ae7337) was the first (ADR 0038). Run 15 (42156838426, on a3df7f5) was the second, and `occt-platforms`, `rust-platforms` and `e2e-engines` moved into `ci.yml` with both rows in wave H (delivery.md 2.14, The move). A failure of `occt-platforms`, `rust-platforms`, `e2e-engines` or `full-rebuild` is fixed in the code or in a test that relied on Linux or Chromium, never with `continue-on-error`, `test.skip` or a condition on the platform, and until then it is recorded in delivery.md open issue 1 or ui.md open issue 41. The one exception is a browser install of `e2e` or `e2e-engines` ending at its 10-minute timeout, or a Tauri install of `rust` or `desktop` running into its job's 60 minutes: that is the Ubuntu mirror, not the code or a test, so it is recorded in delivery.md open issue 16 and the failed jobs are rerun with `gh run rerun <run-id> --failed` (for `e2e` and `e2e-engines`, which upload OCCT's compiled code, within 12 hours of the end of the run's `sources` job, or the whole run with `gh run rerun <run-id>` later) while the run is the newest of its concurrency group in `ci.yml` (delivery.md 2.14 and 3.2); the rerun attempt's result is then the run's, as `gh run list` reports it.
- On Windows, release and CI steps run in Git Bash, whose GNU `link` comes before MSVC's `link.exe` on `PATH`; `bundle` and `rust-platforms` point `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` at the `link.exe` next to `cl.exe`, so rustc does not pick the wrong one.
- On Windows, OCCT, the shim and every Rust crate link the Visual C++ runtime and the C++ standard library statically, and every program takes the Universal C Runtime from Windows (ADR 0038): `scripts/build-occt.sh` sets the static runtime for OCCT, and `.cargo/config.toml` sets it for Rust with the link arguments `/NODEFAULTLIB:libucrt.lib` and `/DEFAULTLIB:ucrt.lib`. A `RUSTFLAGS` variable replaces the configured rustflags and a `RUSTDOCFLAGS` variable the configured rustdocflags. A Windows build with one adds the six flags of the Windows table of `.cargo/config.toml` to it.

## Release checklist

Every release follows these steps (delivery.md 3.6):

1. The source route of ADR 0034 is in place for this commit: the repository is public, so the tag holds the Partsmith source, and `draft` attaches Partsmith's source archive, the OCCT source archive and the dependency source archives (2.13); a release without them stays a draft.
2. CI passed on the `main` commit to release, including `check:notices` (2.4), and a run of the platforms workflow on `main` passed on that commit (2.14): the weekly one, or one started with `gh workflow run platforms.yml --ref main`; `gh run list --workflow platforms.yml --branch main --commit <sha> --status success` shows it; that run's summary shows the full-rebuild median within its budget, or an ADR the owner decided covers that median (2.14, ADR 0054).
3. The version is the same in the workspace `Cargo.toml` (`[workspace.package] version`) and `ui/package.json`, as `verify` checks; a version bump is its own pull request.
4. A release dry run (`workflow_dispatch` of `release.yml`) from `main` passed on that commit (test 13), and each bundle installed and started on its OS (the manual checks of desktop.md section 6).
5. Push the tag `vX.Y.Z` on that commit.
6. When `draft` finishes, download the assets, check them with `sha256sum -c SHA256SUMS`, and install each bundle once more.
7. Edit the draft's notes: the unsigned-bundle paragraph first, then the generated notes.
8. Publish the release.

The unsigned-bundle paragraph, word for word (README.md carries it too):

```text
These builds are not signed yet. On Windows, SmartScreen may say "Windows protected your PC": choose More info, then Run anyway. On macOS, the first open is blocked: open System Settings, then Privacy & Security, and choose Open Anyway for Partsmith. On Linux, the .deb and .rpm need glibc 2.39 or later and WebKitGTK 4.1, as on Ubuntu 24.04. To check a download, run sha256sum -c SHA256SUMS --ignore-missing in the folder that holds it.
```
