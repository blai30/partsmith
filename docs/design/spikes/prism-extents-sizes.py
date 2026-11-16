#!/usr/bin/env python3
"""Builds the prism-extents spike's web core once per construction (kernel.md section 8.1 items 2, 4 and 5).

For each build below, crates/partsmith-wasm/src/main.rs keeps only the calls of the named facade
functions in the `results` array of `keep_prism_extents_spike` and drops the others,
scripts/build-wasm.sh builds the core, and the size and SHA-256 of
ui/public/core/partsmith_core.wasm are printed with the size less the base's. main.rs is restored
byte for byte after each build. The last build is the committed main.rs, which calls every
construction, so the core left in ui/public/core is the branch's own.

Usage: python3 -I prism-extents-sizes.py <worktree>
It needs what scripts/build-wasm.sh needs (an activated Emscripten SDK and
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN) and CARGO_TARGET_DIR.
"""

import hashlib
import re
import subprocess
import sys
from pathlib import Path

# The same build at the base commit 2c56ff9, in the spike's worktree.
BASE_BYTES = 19_996_965

# Each build's name, the facade functions whose calls main keeps (None keeps every call), and how
# many calls that leaves.
BUILDS = [
    ("extrude_feature only", {"extrude_feature"}, 3),
    ("extrude_split only", {"extrude_split"}, 2),
    ("extrude_drafted only", {"extrude_drafted"}, 2),
    ("extrude_lofted only", {"extrude_lofted"}, 2),
    ("extrude_split and extrude_drafted", {"extrude_split", "extrude_drafted"}, 4),
    ("whole branch (every construction)", None, 9),
]

OPENING = "    let results = [\n"
CLOSING = "    ];\n"


def calls(array_body):
    """The top-level elements of the array, each with its trailing comma and newline."""
    elements = []
    depth = 0
    start = 0
    for position, character in enumerate(array_body):
        if character in "([{":
            depth += 1
        elif character in ")]}":
            depth -= 1
        elif character == "," and depth == 0:
            end = array_body.index("\n", position) + 1
            elements.append(array_body[start:end])
            start = end
    if array_body[start:].strip():
        raise SystemExit("the results array has an element without a trailing comma")
    return elements


def variant(source, kept, expected):
    """main.rs with only the calls of the functions in `kept` in the results array."""
    head, rest = source.split(OPENING, 1)
    body, tail = rest.split(CLOSING, 1)
    elements = calls(body)
    if kept is not None:
        elements = [
            element
            for element in elements
            if re.match(r"\s*(\w+)\(", element).group(1) in kept
        ]
    if len(elements) != expected:
        raise SystemExit(f"expected {expected} calls, kept {len(elements)}")
    return head + OPENING + "".join(elements) + CLOSING + tail, elements


def main():
    worktree = Path(sys.argv[1]).resolve()
    main_rs = worktree / "crates/partsmith-wasm/src/main.rs"
    core = worktree / "ui/public/core/partsmith_core.wasm"
    original = main_rs.read_bytes()
    try:
        for name, kept, expected in BUILDS:
            text, elements = variant(original.decode("utf-8"), kept, expected)
            main_rs.write_bytes(text.encode("utf-8"))
            print(f"== {name}: main calls", flush=True)
            for element in elements:
                print("   " + " ".join(element.split()), flush=True)
            subprocess.run([str(worktree / "scripts/build-wasm.sh")], cwd=worktree, check=True)
            data = core.read_bytes()
            print(
                f"== {name}: {len(data)} bytes, delta {len(data) - BASE_BYTES}, "
                f"sha256 {hashlib.sha256(data).hexdigest()}",
                flush=True,
            )
            main_rs.write_bytes(original)
    finally:
        main_rs.write_bytes(original)


if __name__ == "__main__":
    main()
