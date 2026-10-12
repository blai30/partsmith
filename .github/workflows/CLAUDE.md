# .github/workflows

CI, the Cloudflare Pages deploy and the desktop release. GitHub runs only the `.yml` files here.

Spec: `docs/spec/delivery.md` (the jobs of 2.4 and 2.5, the deploy of 3.3, the release of 3.6). Decisions: ADR 0003 (OCCT builds and caches), ADR 0009 (the web core module), ADR 0013 (oxc tooling), ADR 0014 (Tauri and `tauri-cli`), ADR 0017 (the web build and its size gate).

## Interface

- `ci.yml`, on pushes to `main` and pull requests: `occt-native`, `occt-wasm`, `kernel-wasm`, `rust`, `deny`, `ui`, `schema`, `core-wasm`, `web`, `e2e`, `python`, `desktop`, then `deploy`. Artifacts: `core-wasm` (`ui/public/core/`), `web-dist` (`ui/dist/`), `playwright-report` on an end-to-end failure.
- `deploy` publishes `web-dist` with Wrangler: production on `main`, a preview per pull request. It needs the secrets `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` and reads the optional variable `CLOUDFLARE_PAGES_PROJECT` (default `partsmith`); without the secrets it succeeds with a "Deploy skipped" notice.
- `release.yml`, on a `vX.Y.Z` tag: `verify` (the tag equals the version of `partsmith-desktop` and `ui/package.json`, and CI passed for the commit), `bundle` (Linux, Windows and macOS bundles as `bundle-<runner.os>`), `draft` (a draft GitHub Release with the bundles and `SHA256SUMS`, published by a human). A manual run (`workflow_dispatch`) is a dry run that only builds the bundles.

## Commands

```sh
# The distribution check and its tests (run in ui/)
pnpm test:scripts
pnpm build && pnpm check:dist
```

## Rules

- Every `uses:` names an exact release (delivery.md 2.2); tools are pinned where 2.2 says.
- Jobs that need OCCT restore its cache with `fail-on-cache-miss: true`; only `occt-native`, `occt-wasm` and the release `bundle` job build it.
- Every job that runs cargo runs `rustup show active-toolchain || rustup toolchain install` and then `Swatinem/rust-cache`; every job that runs pnpm sets up pnpm from `ui/package.json` and Node 22 with the pnpm cache.
- No secret other than the two Cloudflare secrets and `GITHUB_TOKEN`. Signing and updater secrets wait for a human decision (delivery.md 1.2).
