#!/usr/bin/env bash
# Checks the imports of the Windows program <executable> with dumpbin (delivery.md section 3.6, The check of the
# imports). It prints the listing. It exits 0 when the program imports a DLL of Windows' Universal C Runtime and no DLL
# of the Visual C++ Redistributable. Otherwise it prints the error of delivery.md section 4 as a GitHub annotation, last
# on standard output, and exits 1. It needs dumpbin on PATH, which ilammy/msvc-dev-cmd puts there, and runs in Git Bash.
#
# Usage: scripts/check-windows-imports.sh <executable>
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: scripts/check-windows-imports.sh <executable>" >&2
  exit 2
fi
executable="$1"

# Options start with -, since Git Bash turns an argument that starts with / into a path. dumpbin's exit status is not
# used: the listing decides, and a missing dumpbin lists nothing.
listing="$(dumpbin -nologo -dependents "$executable" | tr -d '\r')" || true
echo "$listing"
# Each line of the listing without its leading spaces.
lines="$(sed 's/^ *//' <<< "$listing")"
if ! grep -q -x -F 'Image has the following dependencies:' <<< "$lines"; then
  echo "::error title=Imports not checked::dumpbin listed no imports of $executable, so the C runtime it imports was not checked. The steps that set up MSVC and build the program run before this one (docs/spec/delivery.md section 3.6)."
  exit 1
fi
# The DLLs of the Visual C++ Redistributable, delay-loaded ones included, in the listing's order. The digit after the
# prefix leaves out msvcp_win.dll and msvcrt.dll, which are part of Windows.
mapfile -t runtime < <(grep -i -E '^(msvcp|msvcr|vcruntime|concrt|vcomp|vccorlib|vcamp|mfc|mfcm)[0-9][^ ]*\.dll$' <<< "$lines")
if [ "${#runtime[@]}" -gt 0 ]; then
  names="$(printf ', %s' "${runtime[@]}")"
  echo "::error title=Visual C++ runtime imported::$executable imports ${names#, }, which only the Visual C++ Redistributable installs. On Windows, OCCT, the shim and every Rust crate link the Visual C++ runtime and the C++ standard library statically (docs/spec/delivery.md section 3.6)."
  exit 1
fi
# The API sets of Windows' Universal C Runtime, which ucrt.lib imports. A program that links the static libucrt.lib
# imports none. Neither ucrtbase.dll nor the API sets of Windows' core count.
if ! grep -q -i -E '^api-ms-win-crt-[a-z0-9-]+\.dll$' <<< "$lines"; then
  echo "::error title=Universal C Runtime not imported::$executable imports no api-ms-win-crt DLL. Every Windows program takes the Universal C Runtime from Windows through ucrt.lib, and .cargo/config.toml links ucrt.lib in place of the static libucrt.lib (docs/spec/delivery.md section 3.6)."
  exit 1
fi
