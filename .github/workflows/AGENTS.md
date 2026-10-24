# .github/workflows

CI, the Cloudflare Pages deploy, the desktop release, the nightly property run and the weekly platforms run. GitHub runs only the `.yml` files here.

Spec: `docs/spec/delivery.md` (the jobs of 2.4 and 2.5, the notices of 2.9, the npm license check of 2.10, the nightly run of 2.11, the platforms run of 2.14, the deploy of 3.3, the release and its checklist of 3.6). Decisions: ADR 0003 (OCCT builds and caches), ADR 0009 (the web core module), ADR 0013 (oxc tooling), ADR 0014 (Tauri and `tauri-cli`), ADR 0017 (the web build and its size gate), ADR 0033 (notices and the npm license check from the lock files), ADR 0034 (OCCT linking and the source route that releases wait for), ADR 0038 (the platforms run outside the gate, and how its jobs move into CI).

## Interface

- `ci.yml`, on pushes to `main` and pull requests: `occt-native`, `occt-wasm`, `kernel-wasm`, `rust`, `deny`, `ui`, `schema`, `core-wasm`, `web`, `e2e`, `python`, `desktop`, then `deploy`. Artifacts, each kept 7 days: `core-wasm` (`ui/public/core/` and `ui/public/THIRD-PARTY-NOTICES.txt`, rooted at `ui/public/`), `web-dist` (`ui/dist/`, with the source stamp `SOURCE.txt` that `web` writes, delivery.md 2.13), `playwright-report` on an end-to-end failure.
- `ui` ends with `pnpm check:licenses`, which holds the npm packages that ship to the allow list of `deny.toml`. `schema` installs the packages first and ends with `pnpm -C ui check:notices`, which runs after a failure of the schema steps too, as long as the install passed. `desktop` checks that the `.deb` holds `THIRD-PARTY-NOTICES.txt`, equal to the repository's file.
- `python` installs CPython 3.13 and 3.12 (3.12 last, so it is the default `python`), makes its virtual environment with `python3.12`, runs `pytest python/tests` and then `pytest python/tests -m packaging` (python-api.md test 76: one abi3 wheel, installed and imported with `python3.12` and `python3.13`).
- `deploy` publishes `web-dist` with Wrangler: production on `main`, a preview per pull request, and fails without deploying unless `ui/dist/SOURCE.txt` names the run's commit (delivery.md 2.13). It needs the secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` and reads the optional variable `CLOUDFLARE_PAGES_PROJECT` (default `partsmith`); without the secrets it succeeds with a "Deploy skipped" notice.
- `release.yml`, on a `vX.Y.Z` tag: `verify` (the tag equals the version of `partsmith-desktop` and `ui/package.json`, and CI passed for the commit), `bundle` (Linux, Windows and macOS bundles as `bundle-<runner.os>`, kept 7 days), `draft` (a draft GitHub Release with the bundles, `THIRD-PARTY-NOTICES.txt`, the OCCT source archive `occt-<version>-source.tar.gz` that `scripts/occt-source-archive.sh` writes, and `SHA256SUMS`, published by a human). A manual run (`workflow_dispatch`) is a dry run that only builds the bundles.
- `nightly.yml`, at 04:23 UTC (cron `23 4 * * *`) and on `workflow_dispatch`: the job `properties` runs the sketch property tests with `PROPTEST_CASES=4096` in release. A scheduled run first asks `gh run list` for the newest finished nightly run on `main`, whatever its conclusion, and when that run tested the head of `main` it stops with a "Nightly skipped" notice before checkout; when `gh` fails, it warns and tests. A manual run always tests.
- `platforms.yml`, on `workflow_dispatch` and on Mondays at 05:41 UTC (cron `41 5 * * 1`), outside the gate: no job of `ci.yml` or `release.yml` waits for it and none is in `deploy.needs`. `occt-platforms` restores or builds OCCT on `windows-2025` and `macos-15` with the steps and key of `release.yml`'s `bundle` job; `rust-platforms`, once both OCCT rows have ended, runs `cargo test --no-fail-fast -p partsmith-occt-sys -p partsmith-engine -p partsmith-desktop`, the examples runner and clippy for those three crates there; `occt-wasm` and `core-wasm` build the web core as CI does; `e2e-engines` runs the tests tagged `@engines` in Firefox and WebKit (`pnpm e2e --project firefox`, ui.md 6.8). Artifacts, each kept 7 days: `core-wasm`, and `playwright-report-<browser>` on an end-to-end failure.

## Commands

```sh
# The scripts CI runs and their tests, and the tests of these workflow files (run in ui/)
pnpm test:scripts
pnpm check:licenses
# pnpm notices rewrites both notices files
pnpm check:notices
pnpm build && pnpm check:dist

# The packaging test of the python job (needs python3.12 and python3.13 on PATH)
pytest python/tests -m packaging

# The nightly property run
PROPTEST_CASES=4096 cargo test -p partsmith-sketch --release --test integration properties::

# The platforms run on main (release checklist item 3), and its latest runs, which promotion reads
gh workflow run platforms.yml --ref main
gh run list --workflow platforms.yml --branch main --limit 2
gh run list --workflow platforms.yml --branch main --commit <sha> --status success

# The engine subset as far as this machine can run it: in Chromium, and the listings of Firefox and WebKit (run in ui/)
pnpm e2e --project chromium --grep @engines
pnpm e2e --grep E-157

# Release checklist items 5 to 7: the dry run from main, the tag, and the draft's assets checked
gh workflow run release.yml --ref main
git tag vX.Y.Z <commit> && git push origin vX.Y.Z
gh release download vX.Y.Z --dir release-vX.Y.Z && (cd release-vX.Y.Z && sha256sum -c SHA256SUMS)
```

## Rules

- Every `uses:` names an exact release (delivery.md 2.2); tools are pinned where 2.2 says.
- Jobs that need OCCT restore its cache with `fail-on-cache-miss: true`; only `occt-native` and `occt-wasm` (of `ci.yml` and of `platforms.yml`), `occt-platforms` and the release `bundle` job build it. `occt-platforms` and `bundle` share the OCCT steps and cache key, so a fix one of them needs applies to the other, and either restores what the other saved on `main`.
- Every job that runs cargo runs `rustup show active-toolchain || rustup toolchain install` and then `Swatinem/rust-cache`; every job that runs pnpm sets up pnpm from `ui/package.json` and Node 22 with the pnpm cache.
- No secret other than the two Cloudflare secrets and `GITHUB_TOKEN`. Signing and updater secrets wait for a human decision (delivery.md 1.2).
- Every job sets `timeout-minutes` (delivery.md 2.4, 2.5, 2.11 and 2.14), so a hung job ends long before GitHub's default of 360 minutes: 150 for the jobs that may build OCCT (`occt-native`, `occt-wasm`, `occt-platforms`, `bundle`), 20 for the nightly `properties` and 60 for every other job, a new one included; `ui/scripts/workflows.test.mjs` checks every value. The repository is public, so standard GitHub-hosted runners bill nothing (ADR 0034). `nightly.yml` also skips nights when `main` has not changed and needs no OCCT; its cron is the one place to change its cadence (delivery.md 2.11).
- Before each tag, start the release dry run from `main`. It proves the bundles (delivery.md section 6, test 13) and saves the Windows and macOS caches on `main`, the only scope a tag run can restore them from; a tag run's own caches are scoped to its tag, and GitHub drops caches unused for 7 days. The weekly platforms run restores the Windows and macOS OCCT caches too, which usually keeps them alive (delivery.md 3.2). The `bundle` job saves its Rust cache only on `main`.
- A job of `platforms.yml`, or one row of its matrix, moves into `ci.yml` once it has passed in the two latest runs of `platforms.yml` on `main` (`gh run list --workflow platforms.yml --branch main`), with the pull request that delivery.md 2.14 (promotion) describes: the job moves as it is with the jobs it needs, joins `deploy.needs`, and every text and test that names it changes with it. A failure in `platforms.yml` is fixed in the code or in a test that relied on Linux or Chromium, never with `continue-on-error`, `test.skip` or a condition on the platform, and until then it is recorded in delivery.md open issue 1 or ui.md open issue 41.
- On Windows, release and platforms steps run in Git Bash, whose GNU `link` comes before MSVC's `link.exe` on `PATH`; `bundle` and `rust-platforms` point `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER` at the `link.exe` next to `cl.exe`, so rustc does not pick the wrong one.

## Release checklist

Every release follows these steps (delivery.md 3.6):

1. The source route of ADR 0034 is in place for this commit: the repository is public, so the tag holds the Partsmith source, and `draft` attaches the OCCT source archive (2.13); a release without that archive stays a draft.
2. Until the AppImage question of ADR 0034 is decided, no release whose assets include an `.AppImage` is published, and no release run, dry run or tag, starts while the Linux row of the matrix builds one: the repository is public, so any signed-in GitHub user can download a run's `bundle-Linux` artifact.
3. CI passed on the `main` commit to release, including `check:notices` (2.4), and a run of the platforms workflow on `main` passed on that commit (2.14): the weekly one, or one started with `gh workflow run platforms.yml --ref main`; `gh run list --workflow platforms.yml --branch main --commit <sha> --status success` shows it.
4. The version is the same in the workspace `Cargo.toml` (`[workspace.package] version`) and `ui/package.json`, as `verify` checks; a version bump is its own pull request.
5. A release dry run (`workflow_dispatch` of `release.yml`) from `main` passed on that commit (test 13), and each bundle installed and started on its OS (the manual checks of desktop.md section 6).
6. Push the tag `vX.Y.Z` on that commit.
7. When `draft` finishes, download the assets, check them with `sha256sum -c SHA256SUMS`, and install each bundle once more.
8. Edit the draft's notes: the unsigned-bundle paragraph first, then the generated notes.
9. Publish the release.

The unsigned-bundle paragraph, word for word (README.md carries it too):

```text
These builds are not signed yet. On Windows, SmartScreen may say "Windows protected your PC": choose More info, then Run anyway. On macOS, the first open is blocked: open System Settings, then Privacy & Security, and choose Open Anyway for Partsmith. On Linux, the .deb, .rpm and .AppImage need glibc 2.39 or later and WebKitGTK 4.1, as on Ubuntu 24.04. To check a download, run sha256sum -c SHA256SUMS --ignore-missing in the folder that holds it.
```
