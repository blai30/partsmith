#!/usr/bin/env python3
"""Builds the projection spike's web core once per construction (kernel.md section 8.1 items 2, 4
and 5, and section 8.7).

crates/partsmith-wasm/src/main.rs calls each of the nine constructions once, in nine lines
`let _ = black_box(...);` of `keep_projection_spike`: each facade function with each value of its
option. For each build below, main.rs keeps only the lines of the named constructions and drops
the others, scripts/build-wasm.sh builds the core, and the size and SHA-256 of
ui/public/core/partsmith_core.wasm are printed with the size less the base's. A build that keeps
one value of an option lets the optimizer drop the code of the other value, so each build measures
its construction alone. main.rs is restored byte for byte after each build. The last build is the
committed main.rs, which calls all nine, so it keeps both paths of every option and the core left
in ui/public/core is the branch's own.

Usage: python3 -I projection-sizes.py <worktree>
It needs what scripts/build-wasm.sh needs (an activated Emscripten SDK and
OCCT_ROOT_WASM32_UNKNOWN_EMSCRIPTEN) and CARGO_TARGET_DIR.
"""

import hashlib
import subprocess
import sys
from pathlib import Path

# The same build at the base commit 2c56ff9, in the spike's worktree.
BASE_BYTES = 19_996_965

# The line of main.rs that calls each construction.
CALLS = {
    "curve-on-plane": "    let _ = black_box(project_on_plane(&block, &edges, &plane, false, &interrupt));\n",
    "curve-on-plane-keeping-parameters": "    let _ = black_box(project_on_plane(&block, &edges, &plane, true, &interrupt));\n",
    "prism": "    let _ = black_box(project_by_prism(&block, &edges, &plane, &interrupt));\n",
    "section-approximated": "    let _ = black_box(section_by_plane(&block, &plane, true, &interrupt));\n",
    "section-exact": "    let _ = black_box(section_by_plane(&block, &plane, false, &interrupt));\n",
    "silhouette-exact": "    let _ = black_box(silhouette_exact(&cylinder, &plane, false, &interrupt));\n",
    "silhouette-exact-with-edges": "    let _ = black_box(silhouette_exact(&cylinder, &plane, true, &interrupt));\n",
    "silhouette-mesh": "    let _ = black_box(silhouette_from_mesh(&cylinder, &plane, false, &interrupt));\n",
    "silhouette-mesh-with-edges": "    let _ = black_box(silhouette_from_mesh(&cylinder, &plane, true, &interrupt));\n",
}

# Each build's name and the constructions whose calls main keeps.
BUILDS = [
    ("curve-on-plane only (GeomProjLib, KeepParametrization off)", {"curve-on-plane"}),
    ("curve-on-plane-keeping-parameters only (GeomProjLib, KeepParametrization on)", {"curve-on-plane-keeping-parameters"}),
    ("prism only (BRepProj_Projection)", {"prism"}),
    ("section-approximated only (BRepAlgoAPI_Section, approximation on)", {"section-approximated"}),
    ("section-exact only (BRepAlgoAPI_Section, approximation off)", {"section-exact"}),
    ("silhouette-exact only (HLRBRep_Algo + OutLineVCompound, as named)", {"silhouette-exact"}),
    ("silhouette-exact-with-edges only (HLRBRep_Algo + OutLineVCompound3d projected, attempt 2)", {"silhouette-exact-with-edges"}),
    ("silhouette-mesh only (HLRBRep_PolyAlgo + PolyHLRToShape::OutLineVCompound, as named)", {"silhouette-mesh"}),
    ("silhouette-mesh-with-edges only (HLRBRep_PolyAlgo segments, attempt 2)", {"silhouette-mesh-with-edges"}),
    ("whole branch (all nine, the committed main)", set(CALLS)),
]


def variant(source, kept):
    """main.rs with only the calls of the constructions in `kept`."""
    for construction, line in CALLS.items():
        if source.count(line) != 1:
            raise SystemExit(f"main.rs does not call {construction} once as expected")
        if construction not in kept:
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
