#!/usr/bin/env python3
"""Builds the rib spike's web core once per construction (kernel.md section 8.1 items 2, 4 and 5).

For each build below, crates/partsmith-wasm/src/main.rs keeps only the calls of the named facade
functions among the two lines `let _ = black_box(rib_...(...));` of `keep_rib_spike` and drops the
other, scripts/build-wasm.sh builds the core, and the size and SHA-256 of
ui/public/core/partsmith_core.wasm are printed with the size less the base's. main.rs is restored
byte for byte after each build. The last build is the committed main.rs, which calls both
constructions, so the core left in ui/public/core is the branch's own.

Usage: python3 -I rib-sizes.py <worktree>
It needs what scripts/build-wasm.sh needs (an activated Emscripten SDK and
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN) and CARGO_TARGET_DIR.
"""

import hashlib
import subprocess
import sys
from pathlib import Path

# The same build at the base commit 2c56ff9, in the spike's worktree.
BASE_BYTES = 19_996_965

# The lines of main.rs that call each construction.
CALLS = {
    "rib_linear_form": "    let _ = black_box(rib_linear_form(&bracket.shape, &spec, true, &interrupt));\n",
    "rib_thickened": "    let _ = black_box(rib_thickened(&bracket.shape, &spec, &interrupt));\n",
}

# Each build's name and the facade functions whose calls main keeps.
BUILDS = [
    ("rib_linear_form only, construction (a)", {"rib_linear_form"}),
    ("rib_thickened only, construction (b)", {"rib_thickened"}),
    ("whole branch (both constructions)", {"rib_linear_form", "rib_thickened"}),
]


def variant(source, kept):
    """main.rs with only the calls of the functions in `kept`."""
    for function, line in CALLS.items():
        if source.count(line) != 1:
            raise SystemExit(f"main.rs does not call {function} once as expected")
        if function not in kept:
            source = source.replace(line, "")
    return source


def main():
    worktree = Path(sys.argv[1]).resolve()
    main_rs = worktree / "crates/partsmith-wasm/src/main.rs"
    core = worktree / "ui/public/core/partsmith_core.wasm"
    original = main_rs.read_bytes()
    try:
        for name, kept in BUILDS:
            main_rs.write_text(variant(original.decode(), kept))
            subprocess.run(["scripts/build-wasm.sh"], cwd=worktree, check=True)
            data = core.read_bytes()
            print(
                f"size {name}: {len(data)} bytes, delta {len(data) - BASE_BYTES}, "
                f"sha256 {hashlib.sha256(data).hexdigest()}",
                flush=True,
            )
            main_rs.write_bytes(original)
    finally:
        main_rs.write_bytes(original)


if __name__ == "__main__":
    main()
