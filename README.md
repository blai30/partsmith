# Partsmith

Open-source parametric CAD for people who design parts for 3D printing. Sketch, model and prepare prints in one app that runs on the desktop and in the browser from one shared core. Every model is a readable text recipe that works with git, and every feature can be edited with the mouse or as Python.

> Status: early development (phase 0). See the [technical design](docs/design/technical-design.md) and the [architecture](docs/design/architecture.md).

## Building

Partsmith links the OCCT geometry kernel, pinned to the version in `scripts/occt-version.sh` and built from source:

```sh
scripts/build-occt.sh native /opt/occt/7.9.3/native
export OCCT_ROOT=/opt/occt/7.9.3/native
cargo test --workspace
```

The web app:

```sh
pnpm -C ui install
pnpm -C ui dev
```

## License

Partsmith is licensed under the [Apache License 2.0](LICENSE), which covers the code, the file-format specification, the examples and the test corpus. Models you make with Partsmith belong to you.

OCCT is used under the LGPL 2.1 with the OCCT exception, as a separately buildable library.
