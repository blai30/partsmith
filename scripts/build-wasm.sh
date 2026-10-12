#!/usr/bin/env bash
# Builds the web core module (crate partsmith-wasm, binary partsmith_core) and copies it into
# ui/public/core/ as partsmith_core.mjs and partsmith_core.wasm (protocol.md section 2.10, ADR 0009).
#
# Usage: scripts/build-wasm.sh
#
# Activates nothing itself. It expects:
# - an activated Emscripten SDK of the version in scripts/emsdk-version.sh (emcc on PATH),
#   for example after `source <emsdk>/emsdk_env.sh`;
# - OCCT built with `scripts/build-occt.sh wasm <prefix>`, named by OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN;
# - the Rust target (`rustup target add wasm32-unknown-emscripten`).
#
# The module's link arguments live in crates/partsmith-wasm/build.rs.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
source "$root/scripts/emsdk-version.sh"

if ! command -v emcc >/dev/null 2>&1; then
  echo "emcc is not on PATH. Activate the Emscripten SDK $EMSDK_VERSION first (source <emsdk>/emsdk_env.sh)." >&2
  exit 1
fi
active="$(emcc --version | head -n 1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)"
if [ "$active" != "$EMSDK_VERSION" ]; then
  echo "Emscripten $active is active, but OCCT and the core are built with $EMSDK_VERSION (scripts/emsdk-version.sh)." >&2
  exit 1
fi
occt="${OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN:-}"
if [ -z "$occt" ] || [ ! -f "$occt/lib/libTKernel.a" ]; then
  echo "Set OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN to an OCCT install built with scripts/build-occt.sh wasm <prefix>." >&2
  exit 1
fi

cd "$root"
cargo build -p partsmith-wasm --bin partsmith_core --target wasm32-unknown-emscripten --profile wasm-release

built="${CARGO_TARGET_DIR:-target}/wasm32-unknown-emscripten/wasm-release"
destination="ui/public/core"
mkdir -p "$destination"
# Emscripten names the .wasm inside the loader after the binary, so the .wasm keeps its name.
cp "$built/partsmith_core.js" "$destination/partsmith_core.mjs"
cp "$built/partsmith_core.wasm" "$destination/partsmith_core.wasm"

# Cloudflare Pages serves files of at most 25 MiB (technical design section 4). This line is
# informational; the deploy gate is ui/scripts/check-dist.mjs (delivery.md section 3.5).
wasm="$destination/partsmith_core.wasm"
limit=$((25 * 1024 * 1024))
size=$(($(wc -c <"$wasm")))
gzipped=$(($(gzip -9 -c "$wasm" | wc -c)))
# The Emscripten SDK ships Node (EMSDK_NODE), whose zlib compresses with brotli at the highest quality.
brotli=$("${EMSDK_NODE:-node}" -e '
  const { readFileSync } = require("node:fs")
  const { brotliCompressSync, constants } = require("node:zlib")
  const options = { params: { [constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY } }
  process.stdout.write(String(brotliCompressSync(readFileSync(process.argv[1]), options).length))
' "$wasm")
mebibytes() {
  awk -v bytes="$1" 'BEGIN { printf "%.2f MiB", bytes / 1048576 }'
}
echo "$wasm: $size bytes ($(mebibytes "$size")), $gzipped bytes with gzip -9 ($(mebibytes "$gzipped")), $brotli bytes with brotli ($(mebibytes "$brotli")); Cloudflare Pages limit: $limit bytes ($(mebibytes "$limit")) per file"
if [ "$size" -gt "$limit" ]; then
  echo "warning: partsmith_core.wasm is larger than the Cloudflare Pages per-file limit." >&2
fi
