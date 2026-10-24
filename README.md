# Partsmith

Open-source parametric CAD for people who design parts for 3D printing. Sketch, model and prepare prints in one app that runs on the desktop and in the browser from one shared core. Every model is a readable text recipe that works with git, and every feature can be edited with the mouse or as Python.

> Status: milestone 1 in progress. What remains before the first release is in [the M1 gap list](docs/design/m1-gap-list.md).

## Try it

**On the web.** The web app deploys to Cloudflare Pages from `main` once CI passes there; its address goes here with the first production deployment. Each pull request gets a preview, linked from the pull request. The web app runs in the current Chrome, Edge, Firefox and Safari on desktop computers, from Chrome and Edge 114, Firefox 128 and Safari 17, and every change is tested in Chromium and every week in Firefox and WebKit.

**On the desktop.** Desktop builds appear on [GitHub Releases](https://github.com/blai30/partsmith/releases) once the first release is published; until then there is nothing to download. Each release holds:

- Windows x64: `Partsmith_{version}_x64-setup.exe`, a per-user installer that downloads WebView2 when it is missing.
- macOS 13 or later on Apple silicon, with Safari 17 or later, whose engine the app draws with: `Partsmith_{version}_aarch64.dmg`.
- Linux x86_64 with glibc 2.39 or later and WebKitGTK 4.1, such as Ubuntu 24.04: `Partsmith_{version}_amd64.deb`, `Partsmith-{version}-1.x86_64.rpm` or `Partsmith_{version}_amd64.AppImage`.

These builds are not signed yet. On Windows, SmartScreen may say "Windows protected your PC": choose More info, then Run anyway. On macOS, the first open is blocked: open System Settings, then Privacy & Security, and choose Open Anyway for Partsmith. On Linux, the .deb, .rpm and .AppImage need glibc 2.39 or later and WebKitGTK 4.1, as on Ubuntu 24.04. To check a download, run sha256sum -c SHA256SUMS --ignore-missing in the folder that holds it.

## Building

CI builds and tests every pull request from source on Linux. The [platforms workflow](.github/workflows/platforms.yml) builds and tests Windows and macOS every week, and the release workflow builds the Windows and macOS apps. On Linux you need:

- Git, CMake, Ninja and a C++17 compiler, for OCCT.
- [rustup](https://rustup.rs), which installs the Rust of `rust-toolchain.toml` and its WebAssembly target (`rustup toolchain install` in the repository).
- Node 22 and pnpm 10.28.0 (`corepack enable && corepack install -g pnpm@10.28.0`).
- Python 3.12 or later, for the Python package.

On Ubuntu 24.04 one line installs the system packages, including the WebKitGTK, xdo, SSL and rsvg libraries the desktop app needs:

```sh
sudo apt-get update && sudo apt-get install -y build-essential cmake ninja-build python3-venv libwebkit2gtk-4.1-dev libxdo-dev libssl-dev librsvg2-dev
```

Partsmith links the OCCT geometry kernel, pinned to the version in `scripts/occt-version.sh` and built from source:

```sh
# Once: the build ends by installing into its prefix, and /opt belongs to root
sudo mkdir -p /opt/occt && sudo chown "$USER" /opt/occt

scripts/build-occt.sh native /opt/occt/7.9.3/native
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test --workspace
```

The web app's geometry core is the same Rust core built for WebAssembly, with the Emscripten SDK of `scripts/emsdk-version.sh`. Without it the web app opens, but its core does not load.

```sh
# Once, in a clone of https://github.com/emscripten-core/emsdk
./emsdk install 4.0.15 && ./emsdk activate 4.0.15 && source ./emsdk_env.sh

# In the repository, with the SDK active: OCCT for WebAssembly, then the core into ui/public/core/
scripts/build-occt.sh wasm /opt/occt/7.9.3/wasm
export OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN=/opt/occt/7.9.3/wasm
scripts/build-wasm.sh
```

## Running

Every build that links OCCT (the desktop app, the command line and Python) needs `OCCT_ROOT` set as above.

The web app, at `http://localhost:5173`:

```sh
pnpm -C ui install
pnpm -C ui dev

# The production build, into ui/dist
pnpm -C ui build
```

The desktop app, with the web app's dev server running:

```sh
# Once
cargo install tauri-cli --version 2.12.2 --locked

# In a second terminal, next to pnpm -C ui dev
(cd desktop/src-tauri && cargo tauri dev)

# A bundle for this platform, into target/release/bundle/
pnpm -C ui build
(cd desktop/src-tauri && cargo tauri build)
```

The command line, `cargo run -p partsmith-cli -- <command>`, has five commands:

- `validate`: load models and check their expressions, references, sketches and assets, optionally rebuilding them and checking canonical form.
- `fmt`: rewrite models in canonical form, or check that they are.
- `rebuild`: rebuild a model and report each feature's status, each body's metrics and optional print checks.
- `export`: export a model's bodies as STL, 3MF, OBJ, STEP or Python.
- `schema`: print, write or check the JSON Schemas of the file format and the protocol.

```sh
cargo run -p partsmith-cli -- rebuild examples/box/default.ptsm
cargo run -p partsmith-cli -- --help
```

Python, in a virtual environment (see [the Python API](docs/spec/python-api.md)):

```sh
python3 -m venv .venv && . .venv/bin/activate
pip install maturin pytest
(cd python && maturin develop --release)

# Run a script that makes one document, and save it
python -m partsmith build script.py -o part.ptsm
```

## Contributing

Start with [AGENTS.md](AGENTS.md) for the rules, commands and invariants, then [the spec index](docs/spec/README.md), [the ADR log](docs/adr/README.md), the [technical design](docs/design/technical-design.md) and the [architecture](docs/design/architecture.md). Behavior is specified before code is written.

Some files are generated and committed. Regenerate them with:

```sh
# The expected results of the examples, and their Python twins
PARTSMITH_BLESS=1 cargo test -p partsmith-cli --test integration examples::
PARTSMITH_BLESS=1 pytest python/tests/test_examples.py

# The JSON Schemas and the protocol types of the UI
cargo run -p partsmith-cli -- schema --write && pnpm -C ui generate:protocol

# The third-party notices
pnpm -C ui notices
```

## License

Partsmith is licensed under the [Apache License 2.0](LICENSE), one license for the code, the specifications, the file format, the examples and the test corpus ([ADR 0028](docs/adr/0028-single-license-apache-2.md)). Models you make with Partsmith belong to you.

The third-party software in Partsmith and its licenses are listed in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) for the desktop app and in [ui/public/THIRD-PARTY-NOTICES.txt](ui/public/THIRD-PARTY-NOTICES.txt) for the web app. OCCT 7.9.3 is used under the LGPL 2.1 with the Open CASCADE exception. It is linked statically, and every build comes with a way to its source: each web deployment names the commit it was built from in `/SOURCE.txt`, and each release carries OCCT's source as `occt-7.9.3-source.tar.gz` next to its tag ([ADR 0034](docs/adr/0034-occt-linking-and-lgpl-obligations.md)). The notices say how to build Partsmith with a modified OCCT.
