#!/usr/bin/env bash
# Builds the pinned OCCT release, with Partsmith's patch scripts/occt-<version>.patch applied, as static libraries,
# for the host or for WebAssembly.
#
# Usage: scripts/build-occt.sh native|wasm <install-prefix>
#
# The wasm target expects an activated Emscripten SDK (emcmake on PATH).
# Only the toolkits Partsmith links are requested; CMake pulls in their dependencies.
set -euo pipefail

target="${1:?usage: build-occt.sh native|wasm <install-prefix>}"
prefix="${2:?usage: build-occt.sh native|wasm <install-prefix>}"

scripts="$(cd "$(dirname "$0")" && pwd)"
source "$scripts/occt-version.sh"

work="${OCCT_WORK_DIR:-${TMPDIR:-/tmp}/occt-${OCCT_VERSION}}"
src="$work/src"
build="$work/build-$target"
# Partsmith's fixes to the pinned release (ADR 0073).
patch="$scripts/occt-${OCCT_VERSION}.patch"

# A fresh clone of the tag gets the patch, and so does an unchanged clone of the pinned commit, such as the one
# scripts/occt-source-archive.sh leaves: a Git clone whose checked-out commit is OCCT_COMMIT and whose tracked files
# are that commit's. Any other existing src is used as it is, which is how a modified OCCT is built (delivery.md
# section 2.13).
cloned=false
apply=false
if [ ! -d "$src" ]; then
  git clone --depth 1 --branch "$OCCT_TAG" https://github.com/Open-Cascade-SAS/OCCT.git "$src"
  cloned=true
  apply=true
  commit="$(git -C "$src" rev-parse "$OCCT_TAG^{commit}")"
  if [ "$commit" != "$OCCT_COMMIT" ]; then
    echo "The tag $OCCT_TAG in $src names the commit $commit, not $OCCT_COMMIT, which scripts/occt-version.sh pins. Upstream may have moved the tag; nothing was built or archived from it (docs/spec/delivery.md section 2.13)." >&2
    # So that the next run clones and checks again instead of building what this one refused.
    rm -rf "$src"
    exit 1
  fi
elif [ "$(git -C "$src" rev-parse --verify --quiet HEAD 2>/dev/null)" = "$OCCT_COMMIT" ] &&
  git -C "$src" diff --quiet HEAD --; then
  apply=true
fi
if [ "$apply" = true ] && ! git -C "$src" apply "$patch"; then
  echo "$patch does not apply to the tag $OCCT_TAG in $src; nothing was built from it (docs/adr/0073-a-check-of-step-bytes-and-a-patch-to-occt-7-9-3.md)." >&2
  # A clone this run made goes, as after the commit check. An existing one stays as it was: git apply changes nothing
  # when it fails.
  if [ "$cloned" = true ]; then
    rm -rf "$src"
  fi
  exit 1
fi

toolkits="TKernel TKMath TKG2d TKG3d TKGeomBase TKBRep TKGeomAlgo TKTopAlgo TKPrim TKBO TKBool TKFillet TKOffset TKFeat TKMesh TKShHealing TKDESTEP TKDESTL"

cmake_args=(
  -S "$src" -B "$build" -G Ninja
  -DCMAKE_BUILD_TYPE=Release
  -DINSTALL_DIR="$prefix"
  -DBUILD_LIBRARY_TYPE=Static
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  # Keep range and null checks in release builds so failures surface as exceptions.
  -DBUILD_RELEASE_DISABLE_EXCEPTIONS=OFF
  -DBUILD_MODULE_FoundationClasses=OFF
  -DBUILD_MODULE_ModelingData=OFF
  -DBUILD_MODULE_ModelingAlgorithms=OFF
  -DBUILD_MODULE_Visualization=OFF
  -DBUILD_MODULE_ApplicationFramework=OFF
  -DBUILD_MODULE_DataExchange=OFF
  -DBUILD_MODULE_DETools=OFF
  -DBUILD_MODULE_Draw=OFF
  -DBUILD_ADDITIONAL_TOOLKITS="$toolkits"
  -DBUILD_DOC_Overview=OFF
  -DUSE_TK=OFF
  -DUSE_FREETYPE=OFF
  -DUSE_FREEIMAGE=OFF
  -DUSE_OPENGL=OFF
  -DUSE_GLES2=OFF
  -DUSE_XLIB=OFF
  -DUSE_TBB=OFF
  -DUSE_VTK=OFF
  -DUSE_RAPIDJSON=OFF
  -DUSE_DRACO=OFF
)

# On Windows, OCCT installs into inc/ and win64/vc14/lib/ by default; partsmith-occt-sys expects
# include/opencascade/ and lib/, as on Unix. OCCT links the Visual C++ runtime and the C++ standard library statically
# (/MT), as the shim and every Rust crate do there (.cargo/config.toml). CMake reads CMAKE_MSVC_RUNTIME_LIBRARY only
# under policy CMP0091. OCCT's minimum CMake version leaves that policy unset (delivery.md section 3.6, ADR 0038).
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    cmake_args+=(
      -DINSTALL_DIR_LAYOUT=Unix
      -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded
      -DCMAKE_POLICY_DEFAULT_CMP0091=NEW
    )
    ;;
esac

case "$target" in
  native)
    cmake "${cmake_args[@]}"
    ;;
  wasm)
    # Native wasm exception handling, matching the Rust wasm32-unknown-emscripten target.
    # OCC_CONVERT_SIGNALS puts setjmp inside try blocks, which Emscripten compiles to invalid
    # wasm with -fwasm-exceptions; the shim never installs signal handlers, so it is not needed.
    # Release builds optimize for size to stay under the 25 MiB per-file hosting limit.
    emcmake cmake "${cmake_args[@]}" \
      -DCMAKE_CXX_FLAGS="-fwasm-exceptions -UOCC_CONVERT_SIGNALS" \
      -DCMAKE_C_FLAGS="-fwasm-exceptions -UOCC_CONVERT_SIGNALS" \
      -DCMAKE_CXX_FLAGS_RELEASE="-Os -DNDEBUG" \
      -DCMAKE_C_FLAGS_RELEASE="-Os -DNDEBUG"
    ;;
  *)
    echo "unknown target: $target" >&2
    exit 1
    ;;
esac

cmake --build "$build" --parallel "${OCCT_JOBS:-$(nproc)}"
cmake --install "$build"
