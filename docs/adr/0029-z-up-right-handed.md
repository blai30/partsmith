# 0029: Right-handed Z-up coordinates, with no up-axis option

Status: Accepted (decided by the project owner, 2026-10-13)

## Context

The owner asked whether Partsmith should use Y-up with Z forward, as game engines do, and whether users should be able to choose. The core has used right-handed coordinates with Z up from the start (kernel.md section 5, architecture conventions): the print bed is the XY plane and layers stack along +Z.

## Decision

Coordinates stay right-handed with Z up everywhere: the kernel, the file format, Python, the CLI, examples, exports and the UI. There is no up-axis setting.

- 3D printing is Z-up end to end: G-code moves the nozzle up along Z, every common slicer shows the bed as XY, and the 3MF core specification puts the build platform on the XY plane with Z up. A part modeled upright in Partsmith lands upright on the bed with no rotation on export. CAD tools whose default is Y-up are a well-known cause of parts arriving on their side in slicers.
- "Y up, Z forward" as in Unity is a left-handed system. CAD geometry and OCCT are right-handed, so a left-handed frame would mirror solids and flip normals. A right-handed Y-up frame has +Z pointing toward the viewer, which is not what game-engine users expect either.
- An up-axis option would have to remap every coordinate field, readout, plane name and view while documents, Python scripts and exports stay Z-up, so the numbers a user types would differ from the ones scripts and files use. That costs more clarity than it gives.

## Consequences

- The view cube, grid, origin planes and axis colors (X red, Y green, Z blue) present Z as up.
- If users ask for Y-up later, the option is a new decision with its own ADR, covering how documents, scripts and exports keep one convention.
