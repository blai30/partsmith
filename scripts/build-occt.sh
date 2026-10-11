#!/usr/bin/env bash
# Builds the pinned OCCT release as static libraries, for the host or for WebAssembly.
#
# Usage: scripts/build-occt.sh native|wasm <install-prefix>
#
# The wasm target expects an activated Emscripten SDK (emcmake on PATH).
# Only the toolkits Partsmith links are requested; CMake pulls in their dependencies.
set -euo pipefail

target="${1:?usage: build-occt.sh native|wasm <install-prefix>}"
prefix="${2:?usage: build-occt.sh native|wasm <install-prefix>}"

source "$(dirname "$0")/occt-version.sh"

work="${OCCT_WORK_DIR:-${TMPDIR:-/tmp}/occt-${OCCT_VERSION}}"
src="$work/src"
build="$work/build-$target"

if [ ! -d "$src" ]; then
  git clone --depth 1 --branch "$OCCT_TAG" https://github.com/Open-Cascade-SAS/OCCT.git "$src"
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

case "$target" in
  native)
    cmake "${cmake_args[@]}"
    ;;
  wasm)
    # Native wasm exception handling, matching the Rust wasm32-unknown-emscripten target.
    emcmake cmake "${cmake_args[@]}" \
      -DCMAKE_CXX_FLAGS="-fwasm-exceptions" \
      -DCMAKE_C_FLAGS="-fwasm-exceptions"
    ;;
  *)
    echo "unknown target: $target" >&2
    exit 1
    ;;
esac

cmake --build "$build" --parallel "${OCCT_JOBS:-$(nproc)}"
cmake --install "$build"
