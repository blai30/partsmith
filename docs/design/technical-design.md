# Partsmith: Technical Design for Parametric CAD for 3D Printing

Oct 10, 2026 · @Brian Lai

## 1. Overview and goals

We are building Partsmith, an open-source parametric CAD application for hobbyists and makers who design parts for 3D printing. It runs on desktop and in the browser from one shared core. It matches Fusion and FreeCAD on sketching and part design, and drops everything a print workflow doesn't need. GUI and code are two views of the same model: every feature can be edited with the mouse or as Python. All of its code will be written by AI agents (section 17).

**Goals**

- **Parity:** every sketch and Part Design operation a maker uses in Fusion or FreeCAD is available (see section 8 for the matrix).
- **Print-native:** printability checks, print-aware features and one-click slicer handoff are first-class, not plugins.
- **Minimal UI:** one context-sensitive toolbar, a feature timeline, and a command palette. No workbench switching and no ribbon of 200 icons.
- **Robust models:** edits upstream don't silently break downstream features. Topological naming is solved by design (section 5).
- **Open and diffable:** a documented, text-based native format that works with git.

**Success criteria for v1**

- A Fusion user can model a typical printable part (enclosure, bracket, gear, threaded cap) without reading docs, in no more time than in Fusion.
- Rebuilding a 50-feature part takes under 1 s on a 2022 mid-range laptop; sketch drags stay at 60 fps.
- Fewer than 1% of upstream edits in the regression corpus break a downstream reference.
- Every GUI-built model exports to Python that rebuilds the identical shape.

**Engineering principles**

These bind every change, whoever writes it.

1. **Production-grade, solid as a rock.** Partsmith is professional software that people rely on for real parts. It must be stable, fully tested and deployable in the real world. No hacks, no shortcuts, no "temporary" workarounds left behind.
2. **Testing first.** Many small, simple unit tests, plus automated testing on every change. Untested code doesn't merge.
3. **An example for every feature.** A curated set of example models exercises every feature in the app and runs in CI on every build (section 16).
4. **Correct and idiomatic over clever.** Choose the most correct, clean, modern, industry-standard design. Use a battle-tested design pattern only when it solves a problem the code actually has.
5. **Less is more.** Less code, fewer dependencies, fewer options on screen. Every addition has to earn its place.

## 2. Scope and non-goals

v1 covers sketching, solid part design, multi-body parts, light assemblies, a code workflow and print preparation up to the slicer. Anything after the slicer, and anything not aimed at printing, is out.

**In scope (v1)**

- 2D constraint sketcher, including sketches on faces and planes and projected geometry
- Solid features: extrude, revolve, sweep, loft, holes, threads, fillets, chamfers, shell, draft, patterns, mirror, booleans, split, and others
- Surface basics needed for solids (patch, thicken, offset, ruled). Full freeform surfacing is a stretch goal.
- Parameters and expressions, configurations (design tables)
- Multi-body parts, components, simple joints
- Python API, script features, read-only OpenSCAD import
- Printability analysis, print-aware features, 3MF/STL/STEP export, slicer handoff, for both FDM and resin printers

**Non-goals**

- BIM, CAM/toolpaths, FEM/simulation, rendering/raytracing, sheet metal, PCB/ECAD, 2D drawings with GD&T. A simple dimensioned PDF drawing may come post-v1.
- Motion studies, kinematic solvers, large assemblies (over 200 parts)
- Mesh sculpting. We import meshes and can use them as references or bodies, but we don't edit them freeform.
- Editing .f3d files. Fusion's format is closed; interop goes through STEP.

**Post-v1, but the architecture must allow for it** (section 15)

- Cloud sync and sharing links
- AI assist (natural-language edits, built on the code API)
- Plugin/extension API with a public SDK
- Model library with parametric remixing

## 3. Key decisions

These decisions come from the kickoff interview. Items marked *proposed* are engineering recommendations still open for review.

| Area | Decision | Why |
| --- | --- | --- |
| Name | **Partsmith**. Text source files are `.ptsm`; the zipped sharing bundle is `.ptsz`. | A smith of parts: maker craft plus precision. We found no CAD tool, app or file type using the name. Short three-letter options (.psm, .pts, .prt) are taken by other CAD and point-cloud formats; we found no registered use of .ptsm or .ptsz. A trademark check is still needed before public launch. |
| License | Apache-2.0, one license for the code, the file-format spec, the examples and the test corpus ([ADR 0028](../adr/0028-single-license-apache-2.md)). | A permissive, widely used open license with an explicit patent grant from every contributor, which matters in a patent-heavy field like CAD: anyone may use, modify, sell or embed Partsmith, personally or commercially. Models made with Partsmith belong to their authors. OCCT stays under the LGPL 2.1 with the Open CASCADE exception and is linked statically; publishing the source of every build and OCCT's, so users can relink, meets the LGPL ([ADR 0034](../adr/0034-occt-linking-and-lgpl-obligations.md)). GPL code (SolveSpace solver, OpenSCAD) stays out. |
| Development | All code is written by AI agents (Claude Code), with human approval at gates. | Shapes the repo layout, specs, testing and review process (section 17). |
| Platforms | Desktop (Windows, macOS, Linux) and web, from one shared core. The web build is deployed by CI to Cloudflare Pages. | Desktop runs the core natively and multithreaded; web runs the same core as WASM in a worker. |
| Print technologies | FDM and resin (MSLA/SLA) in v1 | Resin adds hollowing, drain holes and suction-cup checks, roughly 15–20% more print-module work (section 11). |
| Audience | Hobbyist and maker first | Defaults, wording and tutorials target people coming from Tinkercad, Fusion personal use and FreeCAD. |
| Geometry kernel | OpenCascade (OCCT) 7.8+ (*proposed*) | The only mature open B-rep kernel. Used by FreeCAD, CadQuery and build123d. Parasolid is not an option for an open-source project. OCCT stays C++ behind a thin FFI shim; everything else is Rust. |
| Sketch solver | Fork of FreeCAD's PlaneGCS behind our own interface (*proposed*) | Proven and LGPL. Wrapped from Rust, with a native Rust port possible later. The SolveSpace solver is GPLv3, which is incompatible with a permissive license. |
| Native format | Text-based `.ptsm` as source of truth, plus an optional zipped `.ptsz` bundle for sharing | Git-diffable, human-readable, durable (section 6). |
| Interop | STEP import/export, 3MF/STL/OBJ export, mesh import, best-effort `.FCStd` import | STL, 3MF and STEP can't hold parametric history, so they are interchange formats only. `.f3d` is closed. |
| Code workflow | Python, with a build123d-style API | The de facto standard for B-rep code CAD. OpenSCAD `.scad` is supported as an import layer. |
| Assemblies | Multi-body parts plus light assemblies (components, simple joints) | Enough for print-in-place parts and multi-part prints, without a motion solver. |
| Post-v1 | Cloud, AI assist, plugins, model library | Deferred, but with seams designed in now (section 15). |

## 4. System architecture

The system is one Rust core (wrapping the C++ OCCT kernel), compiled natively for desktop and to WebAssembly for the browser, under one TypeScript UI. The UI never touches geometry directly. It sends commands to the core and receives tessellated meshes, selection IDs and diagnostics back. That single boundary is what lets desktop and web share almost everything.

**Core (Rust, shared)**

- **Document model:** parameters, the feature graph, stable IDs, undo/redo as a command log, and serialization to and from `.ptsm`.
- **Recompute engine:** builds the dependency graph, invalidates dirty features, runs them in order, and caches results per feature (section 14).
- **Feature library:** each feature is a pure function from (inputs, parameters) to (shape, naming map). The same interface is used by built-in features, script features and future plugins.
- **Sketcher:** entities, constraints, the solver wrapper and DOF analysis.
- **Geometry layer:** a thin wrapper over OCCT that adds topological naming, tolerance handling and error translation.
- **Print module:** printability analysis, print-aware features, 3MF writer and slicer handoff.
- **Tessellator:** turns B-rep into display meshes with face, edge and vertex IDs for picking.
- **Python bindings:** PyO3 (*proposed*) on desktop; the same API surface exposed to Pyodide on the web.

**Rust and C++ boundary.** Everything we write is Rust: the document model, recompute engine, naming layer, feature library, print module, file I/O, protocol and Tauri host. OCCT and PlaneGCS stay C++ behind a thin shim exposed through the `cxx` crate.

- The shim catches every OCCT exception and returns error codes, so no C++ exception or OCCT type crosses into Rust.
- Shapes are opaque handles with Rust-side reference counting.
- The web build targets `wasm32-unknown-emscripten`, because OCCT requires Emscripten. That toolchain path is less polished than Rust's usual WASM target, so a build spike is the first milestone task (section 18).
- Pure-Rust kernels (Truck, Fornjot) can't yet do fillets, shells and robust booleans. Keeping the kernel behind the shim leaves a later swap possible.

**UI (TypeScript, shared)**

- Framework: SolidJS or React (*proposed*: SolidJS, for fine-grained reactivity in panels with many live values).
- Viewport renderer on WebGPU with a WebGL2 fallback. It is a custom thin renderer, not a game engine. It needs fast picking, edge overlays, section views and analysis color maps.
- The UI talks to the core through one typed message protocol (commands in, events and mesh buffers out), generated from a shared schema.

**Hosts**

- **Desktop:** Tauri 2 shell (*proposed*). The core runs in-process as a native library, with OCCT parallelism enabled. Gives native file dialogs, slicer launching, and an embedded CPython.
- **Web:** static site. The core runs in a Web Worker as WASM, with SharedArrayBuffer for mesh transfer. Files are stored with the File System Access API, plus IndexedDB as a fallback. Python runs in Pyodide, loaded lazily.

**Build and deployment**

- GitHub Actions builds every crate, runs the tests and model corpus, and builds the web app and desktop installers.
- The web app deploys automatically to Cloudflare Pages. Every merge to `main` goes to production, and every pull request gets its own preview URL, so reviewers can try a change in the browser.
- A `_headers` file sets `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Browsers require both before they allow SharedArrayBuffer and WASM threads.
- Pages limits each file to 25 MiB ([Cloudflare limits](https://developers.cloudflare.com/pages/platform/limits/)). A full OCCT WASM build can exceed that, so the core ships as lazily loaded modules (base kernel, booleans and fillets, STEP I/O, mesh tools), each linked with only the OCCT toolkits it needs. Anything still over the limit is served from Cloudflare R2, which Cloudflare recommends for large files.
- The Tauri bundler signs desktop builds for Windows, macOS and Linux and publishes them to GitHub Releases, with auto-update.

The diagram below shows how the layers connect.

*(Diagram in the original document: architecture, UI, Rust core, C++ kernel.)*

The protocol and the FFI shim are the only two boundaries. Everything between them is Rust and identical on both platforms.

## 5. Geometry kernel and topological naming

OCCT supplies the B-rep math. Reliability comes from a naming layer we own. Every reference to a face, edge or vertex is stored as a **semantic query** describing how that element was created, never as an index like `Face7`. Index-based references are the root cause of FreeCAD's long-running topological naming problem.

**Naming model**

1. Every sketch entity, feature and parameter has a permanent UUID.
2. Each feature, after running, emits a **naming map**. Every output sub-shape gets a name derived from its history. OCCT's `BRepTools_History` (Generated / Modified / Deleted) feeds this. Example: `extrude#a1f3 / lateral / sketch#77c0.line#9e12`, or `extrude#a1f3 / end-cap`.
3. A downstream reference (fillet edges, sketch-on-face, hole placement) stores the query string. It also stores a **geometric fingerprint**: surface type, normal or axis, centroid, area or length, and adjacency.
4. On rebuild, the resolver uses an element only when the model's recorded history connects it to the reference: the exact name; every piece of an element that later split; the face that merged faces became; the faces made from the sketch lines that replaced or continued the line it was made from; the rim a shell left where it removed a face; for an edge or a corner, the one the same hole or sketch line makes under another role. For a piece of a split face (a name with `#n`), the reference also stores the faces that piece touched and its siblings did not, so the piece is found by what it touches, whatever the cut order or the sizes. The fingerprint never chooses an element on its own: it confirms the faces of imported geometry, which have no other identity, and decides, for a piece stored without those faces (a reference written by hand, by a script or before they existed), whether the named piece is used as it is or reported as a guess. When nothing in the history connects, the reference is lost, and the elements that keep a face it touched, then those nearest to where it was, are offered as **candidates ranked by fingerprint for the user to choose, never substituted silently** ([ADR 0021](../adr/0021-resolve-references-by-meaning.md), [naming.md](../spec/naming.md) section 3.9).
5. If a reference resolves to nothing or to an ambiguous set, the feature still builds where it can: a lost entry of a list is skipped with a warning, and only a required reference that is lost, or a list with nothing left, fails the feature. The message names the lost element in words and the feature that removed it ("The front top edge of 'Box 1' selected for Edges was cut away by 'Extrude 1', which comes before this fillet."), the app adds the edit that started it when it saw that edit ("This started after you added 'Extrude 1' before this fillet."), and both offer one-click fixes. Every new loss raises a notification, and a **repair mode** shows the lost element's **ghost** where it was in its last build (or a marker at its stored position when the document was opened already broken) and numbered candidates described by what they keep and how far they are ("on the top of 'Box 1', 5 mm from where it was"), never by a match score. One click uses a candidate (one undo entry), and any pick on the model replaces the reference. A reference that followed a change by meaning, such as a sketch line replaced one for one, builds with a notice and "Keep this match"; one that now covers more than was picked, or is a guess, builds with a warning. Nothing is rewritten without the user ([ADR 0024](../adr/0024-diagnostics-with-actions-causes-and-ghosts.md), [the error guidelines](../guidelines/errors.md)). The [reference stability corpus](reference-stability-corpus.md) fixes the result, message and actions of 66 edits made earlier in the timeline, and its scenarios run as tests.

**Kernel hygiene**

- Global modeling tolerance 1e-6 mm; sketch solver tolerance 1e-10.
- Every OCCT call goes through a wrapper that catches `Standard_Failure` and turns it into a user-facing message ("Fillet radius 3 mm is larger than the 2.4 mm adjacent face") instead of a generic failure.
- Shape healing (`ShapeFix`) runs on imported geometry and after booleans.
- Pin a specific OCCT version and track upstream patches in our fork, because kernel upgrades change output topology. Regression tests (section 16) gate every upgrade.

**Defaults that avoid fragile references**

- New sketches attach to origin planes or datum planes unless the user picks a face.
- A sketch on a face keeps its place on that face: the position saved when it is attached is in the coordinates of the face's own feature, so it moves when a Move body, a pattern or a mirror copy moves the face (documents written before format version 2 keep theirs in world coordinates and rebuild as before). If a feature before the sketch removes the face, the sketch stays where that face was just before that feature, read from the rebuild itself. Otherwise (the face is ambiguous or no longer flat, or no earlier state of the body has it because its feature was deleted, is suppressed or now comes later) the sketch uses the position saved with it. Its warning says which of the two it did, and nothing is written to the document until the user re-attaches the sketch ([ADR 0023](../adr/0023-frames-live-on-the-model.md), [features/sketch.md](../spec/features/sketch.md) section 3.3).
- Selection tools favor feature-level choices ("all edges of this extrude's top face") over picking many single edges.

## 6. Document model and native file format

The native file is a canonical, human-readable text file (`.ptsm`) that stores the *recipe* (parameters, sketches and the feature list), never the resulting geometry. Geometry is always regenerated. A zipped `.ptsz` bundle packages the same text plus assets for sharing as one file.

**Why text, and why not STEP, 3MF or STL as the native format.** STL is a bare triangle mesh. 3MF is a mesh plus print metadata. STEP is exact B-rep geometry with no usable feature history. None of them can reopen a model as editable sketches and features, so all three are exports. A parametric recipe is small (typically 10–200 KB), so text costs almost nothing and gives git diffs, blame, merges, hand repair and long-term readability. KiCad, SolveSpace and code-CAD tools follow the same pattern.

**Encoding (*proposed*: canonical JSON)**

- JSON, validated by a published JSON Schema, with a `format_version` field and forward migrations.
- Canonical serializer: fixed key order, 2-space indent, shortest round-trip floats, and items sorted by creation order. Saving an unchanged model produces a byte-identical file.
- Small leaf records (sketch points, lines, constraints) go on one line each, so a single edit shows as a one-line diff.
- References are UUIDs plus the semantic queries from section 5, never array indices.

**Example (trimmed)**

```json
{
  "format": "partsmith", "format_version": 1, "units": "mm",
  "parameters": [
    {"id": "p-wall", "name": "wall", "expr": "2 mm"},
    {"id": "p-clear", "name": "clearance", "expr": "printer.fit.slip"}
  ],
  "bodies": [{"id": "b-1", "name": "Enclosure", "material": "PETG", "color": "#3b82f6"}],
  "features": [
    {"id": "f-sk1", "type": "sketch", "plane": "origin/XY",
     "entities": [
       {"id": "e-1", "type": "rect", "corner": [0, 0], "size": [80, 50]}
     ],
     "constraints": [
       {"id": "c-1", "type": "distance", "on": ["e-1.width"], "expr": "80 mm"}
     ]},
    {"id": "f-ext1", "type": "extrude", "body": "b-1", "profile": "f-sk1/region:0",
     "extent": {"kind": "distance", "expr": "30 mm"}, "op": "new"},
    {"id": "f-shell1", "type": "shell", "body": "b-1", "thickness": "wall",
     "remove_faces": [{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30"}]}
  ]
}
```

**Storage layout**

| File | Contents | In git? |
| --- | --- | --- |
| `part.ptsm` | The recipe (source of truth) | Yes |
| `assets/` | Imported STEP/STL bodies, images, fonts, each referenced by content hash | Yes (Git LFS recommended for large meshes) |
| `.ptsm-cache/` | B-rep per feature, display meshes, thumbnail | No; disposable and regenerated |
| `part.ptsz` | ZIP of the recipe + assets + optional cache, for email or upload | Optional |

**Multi-document and git tooling**

- A project is a folder: parts reference each other by relative path and UUID, and assemblies reference parts the same way.
- A `partsmith validate` CLI and a git merge driver come in v1. The merge driver merges at the JSON-record level and reruns validation, so a merge that creates an over-constrained sketch is flagged instead of saved silently.
- A visual diff (two models overlaid with added and removed volume colored) is planned for post-v1.

**Other formats**

- `.FCStd` import, best-effort: Part Design bodies, sketches and common features map across; unsupported features import as dead solids with a warning. No `.FCStd` write in v1.
- `.f3d` is not supported. Users export STEP from Fusion.

## 7. Sketcher

The sketcher covers the union of Fusion's and FreeCAD's sketch tools. It adopts Fusion's interaction style: type dimensions while drawing, constraints inferred automatically, and color showing what is fully defined. FreeCAD's power tools (external geometry, carbon copy, block constraint) are kept, but tucked behind the command palette.

**Entities and drawing tools**

| Group | Tools |
| --- | --- |
| Lines | Line / polyline (tangent-arc toggle mid-draw), construction line, centerline, midpoint line |
| Rectangles | 2-point, 3-point (rotated), center rectangle, rounded rectangle |
| Circles and arcs | Center–radius, 2-point, 3-point, tangent circle; 3-point arc, center arc, tangent arc |
| Conics and splines | Ellipse, elliptical arc, parabola, hyperbola, conic curve (rho), B-spline by control points, B-spline by fit points (periodic option) |
| Shapes | Regular polygon (inscribed/circumscribed), slots (center-to-center, overall, center-point, arc slot) |
| Other | Point, sketch text (TrueType, along path), canvas image for tracing, DXF/SVG import |

**Modify tools:** trim, extend, split/break, 2D fillet, 2D chamfer, offset (with live distance), mirror, rectangular and circular pattern, pattern along path, move/copy/rotate/scale, and a construction toggle.

**Reference geometry:** project (edges, faces, bodies), intersect body with sketch plane, project silhouette, include from another sketch (carbon copy). Projected geometry stays linked to its source through the naming layer and updates with it. A per-item "break link" option turns it into regular geometry.

**Constraints**

| Kind | Constraints |
| --- | --- |
| Geometric | Coincident, point-on-object, horizontal, vertical, parallel, perpendicular, tangent, smooth (G2 curvature), collinear, concentric, equal, symmetric, midpoint, fix/lock, block (freeze a sub-sketch rigidly) |
| Dimensional | Distance, horizontal/vertical distance, point-to-line, radius, diameter, angle, arc length, angle of a line to an axis |
| Special | Snell's law / refraction (FreeCAD parity), B-spline knot and weight constraints |
| Modes | Driving vs driven (reference) dimensions; every dimension takes an expression or parameter |

**Solver and feedback**

- PlaneGCS backend, with solver choice DogLeg first and Levenberg–Marquardt as fallback. It runs on every drag frame; target time under 4 ms for a 500-constraint sketch.
- DOF display: under-constrained geometry is blue, fully constrained is black (or white in dark mode), and conflicting is red. The sketch header shows the remaining DOF count.
- Conflicting and redundant constraints are named, with one-click removal suggestions.
- Auto-constraint inference while drawing (snaps to horizontal, vertical, coincident, tangent and midpoint), with a hold-key to suppress it.
- "Dimension as you draw": typing a number while drawing creates a driving dimension.
- Profile check: open loops, overlaps and self-intersections are highlighted before you leave the sketch, and closed regions are shaded so you can see what will extrude.
- Sketch-level parameter use: `width = 2 * wall + clearance` works anywhere a number is entered.

## 8. Part design features and parity matrix

v1 ships every solid-modeling feature that Fusion's Solid tab and FreeCAD's Part Design workbench offer. The only exceptions are T-spline Form modeling and advanced freeform surfacing, which move to v1.x or later. Additive and subtractive versions of a feature are one tool with an operation toggle (new body / join / cut / intersect), the way Fusion does it, rather than FreeCAD's separate Pad and Pocket tools.

| Our feature | Fusion | FreeCAD | Release | Notes |
| --- | --- | --- | --- | --- |
| Extrude | Extrude | Pad, Pocket | v1 | One/two sides, symmetric, to face/body/vertex, through all, from offset, taper angle, thin wall |
| Revolve | Revolve | Revolution, Groove | v1 | Angle, full, two sides, to face |
| Sweep | Sweep | Additive/Subtractive Pipe | v1 | Guide rail, twist, scale, orientation modes |
| Loft | Loft | Additive/Subtractive Loft | v1 | Rails, centerline, point ends, tangent/curvature end conditions |
| Helix / coil | Coil | Additive/Subtractive Helix | v1 | Pitch, turns, height, taper, profile from sketch or preset |
| Rib and web | Rib, Web | (via Pad) | v1 | Open-profile thickening to the body |
| Emboss / deboss | Emboss | n/a | v1 | Text and profiles onto curved faces; critical for printed labels |
| Hole | Hole | Hole | v1 | Simple, counterbore, countersink, tapped; on sketch points; ISO/UNC tables plus print presets (section 11) |
| Thread | Thread | (Hole thread) | v1 | Cosmetic or modeled; FDM-tuned profiles |
| Fillet | Fillet, Rule Fillet | Fillet | v1 | Constant, variable radius, chord length, setback corners, G2 option, rule fillet by face group |
| Chamfer | Chamfer | Chamfer | v1 | Equal, two distances, distance–angle |
| Shell | Shell | Thickness | v1 | Inside/outside, per-face thickness |
| Draft | Draft | Draft | v1 | Fixed plane, parting line |
| Primitives | Box, Cylinder, Sphere, Torus, Pipe | Additive/Subtractive primitives | v1 | Box, cylinder, sphere, cone, torus, wedge, prism, pipe |
| Patterns | Rectangular, Circular, Path pattern | Linear, Polar, MultiTransform | v1 | Of features, faces or bodies; skip instances; pattern-of-pattern; driven by parameters |
| Mirror | Mirror | Mirrored | v1 | Features, faces or bodies |
| Boolean | Combine | Boolean | v1 | Join, cut, intersect, keep tools |
| Split | Split Body, Split Face | (Part Slice) | v1 | Split by plane, face or sketch; crucial for printing big parts |
| Scale | Scale | (Part Scale) | v1 | Uniform and non-uniform |
| Move / copy body | Move/Copy | Placement | v1 | Free, point-to-point, along axis, align |
| Direct edits | Press Pull, Offset Face, Replace Face, Delete Face | (limited) | v1 | Recorded as parametric features in the timeline |
| Datum geometry | Construction planes, axes, points | Datum plane/line/point, LCS | v1 | All Fusion construction modes; coordinate systems |
| Derive / shape binder | Derive | Shape Binder, SubShapeBinder | v1 | Live link to geometry in another body or file |
| Surfaces for solids | Patch, Extend, Trim, Stitch, Thicken, Offset, Ruled, Boundary Fill | Part surface tools | v1 (core set) | Enough for blends and closing gaps; advanced surfacing later |
| Parameters and expressions | Change Parameters | Spreadsheet, VarSet, expressions | v1 | Units-aware expressions, functions, cross-file links |
| Configurations | Configurations | (Spreadsheet configs) | v1.1 | Table of parameter sets and feature suppression |
| Suppress, reorder, rollback | Timeline | Tip, tree reorder | v1 | Drag the timeline marker to roll back; drag features to reorder |
| Measure and inspect | Measure, Section Analysis, Interference | Measure, Cross-sections | v1 | Mass properties with print material density |
| Mesh bodies | Mesh workspace, Mesh to BRep | Mesh workbench | v1 (basic) | Import, repair, convert low-poly mesh to solid, boolean with solids |
| Form (T-splines) | Form | n/a | Not planned v1 | Possibly SubD later; not core to print parts |

**Interaction pattern for every feature:** select inputs, see a live preview within 100 ms, adjust with on-canvas handles or the floating panel, and confirm with Enter. Every numeric field accepts an expression. Every feature can be edited later by double-clicking it in the timeline or on the model.

## 9. Multi-body and light assemblies

A single file can hold many bodies and components, placed with a small set of joints, but there is no motion solver. That covers the cases makers actually hit: a box and its lid, print-in-place hinges, parts split to fit the bed, and multi-color prints.

**Bodies**

- A part file holds any number of solid bodies. Every feature declares its target body and operation (new / join / cut / intersect), like Fusion. This avoids FreeCAD's rule of one solid per Body.
- Each body has a name, color, material (PLA, PETG, ABS/ASA, TPU, resin…), visibility and an "export as" setting (separate object, or merged).
- Body-to-body tools: boolean, split, derive, interference check, minimum-gap check (for print-in-place clearance).

**Components**

- A component groups bodies, sketches and features with its own origin. It can be local to the file or linked from another `.ptsm` file, which stays live and updates when the source changes.
- Linked components can be overridden per instance with parameters ("this M3 standoff is 12 mm long") without editing the source.
- Insert from file, a library folder, or a STEP/mesh import.

**Joints (no kinematics)**

- Rigid, revolute, slider, cylindrical, planar and ball joints, defined by picking joint origins on geometry (Fusion style) with offsets and angle limits.
- Joint values are parameters. You can drag a hinge in the viewport or set `lid_angle = 90 deg`, and run an interference check at that pose.
- A simple positional solver places components to satisfy joints. Contact physics, motion studies and drives are out of scope.

**Print-oriented assembly helpers**

- Explode for printing: lays every body flat on a virtual bed, oriented for printing, without changing the design pose (section 11).
- Print-in-place check: confirms each moving joint has at least the configured clearance at every point and flags fused regions.
- Bill of materials: lists bodies and purchased parts (screws, magnets, bearings) as a CSV or a sidebar list.

## 10. Code workflow

Code is a first-class way to model, built on Python with a build123d-style API. The API calls the same Rust feature library the GUI uses, so a script and a mouse-built model produce identical, equally editable features. It supports three modes:

1. **Code-first files:** the whole model is a `.py` script, as in OpenSCAD or CadQuery, with live preview on save.
2. **Script features:** a code block placed in the GUI timeline, with GUI features before and after it.
3. **Export to code:** any GUI model can be exported as a readable script.

**Example**

```python
from partsmith import *

wall = param("wall", 2*mm, min=1.2*mm, max=4*mm)   # shows up as a slider
clear = param("clearance", printer.fit.slip)

with Part("Enclosure", material="PETG") as box:
    with Sketch(Plane.XY) as s:
        Rectangle(80, 50, rounded=4)
    extrude(s, 30)
    shell(faces().top(), thickness=wall)
    hole(box.faces().bottom(), at=grid(2, 2, 60, 30), kind=Hole.counterbore(M3))

with Part("Lid") as lid:
    lip = offset(box.faces().top().outer_wire(), -(wall + clear))
    extrude(lip, 3) + extrude(box.faces().top().outer_wire(), 2)

export_3mf([box, lid], arrange=True)
```

**Design rules**

- **One API, two front ends.** Every GUI feature has a Python constructor with the same parameters, and every feature in the `.ptsm` file can be printed as Python. A round-trip test (GUI → Python → `.ptsm`) is part of CI (section 16).
- **Selectors instead of indices.** `faces().top()`, `edges().parallel_to(Z).longest()` and similar selectors compile to the same semantic queries as GUI picks, so scripts survive upstream edits too.
- **Parameters become UI.** `param()` declarations appear in a Customizer-style panel with sliders, dropdowns and checkboxes, like OpenSCAD's Customizer. Non-coders can then adjust code-built models.
- **How sync works.** GUI edits to a script's parameters write back into the script's `param()` default values. Edits to the geometry a script produced are added as normal GUI features after the script feature, not patched into the code. Full two-way sync of arbitrary code is impossible in general, so this is the limit on purpose.
- **Editor:** built-in code panel (CodeMirror 6) with autocomplete from the API's type stubs, inline errors that point at the line, and a hover on any feature to jump to its line. VS Code users get the same experience through the published `.pyi` stubs and a small extension (post-v1).

**Runtime**

- Desktop: embedded CPython 3.12+ through PyO3; users can `pip install` into a project virtual environment.
- Web: Pyodide in a worker, calling the WASM core through a JS bridge. Pure-Python packages only.
- **Security:** scripts are code. Opening a file that contains scripts from an unknown source shows a trust prompt, and the geometry stays as last cached until the user allows it. On the web, scripts run in the browser sandbox. On desktop, untrusted scripts run with no file or network access by default.

**OpenSCAD compatibility**

- A native `.scad` interpreter written in Rust (we don't bundle OpenSCAD itself, because its GPL license would conflict with ours). It maps primitives, transforms, CSG, `linear_extrude`, `rotate_extrude`, `hull`, `polyhedron`, modules, functions and `include`/`use` to B-rep features.
- `minkowski` and other operations that don't map well to B-rep fall back to mesh with a warning.
- Customizer variables become parameters. An imported `.scad` file can stay linked (re-evaluated on change) or be converted to a Python script feature.

## 11. 3D-printing features

Printing is built into the core, not added as a plugin. A printer profile drives tolerances, checks and export, so the model knows what it will be printed on. This is the main reason to choose this app over Fusion or FreeCAD.

**Printer profiles**

- Technology (FDM or resin), bed size and shape, nozzle diameter, layer height, max overhang angle, max bridge length.
- **Fit classes** (press, snug, slip, loose), each a clearance value. They start as defaults per technology and can be tuned with a small calibration print the app generates. Parameters can reference them (`printer.fit.slip`), so changing printers re-tunes every clearance in the model.
- Profiles can be imported from Bambu Studio, OrcaSlicer and PrusaSlicer configs where possible.

**Printability checks (live, in the viewport)**

| Check | What it shows | Default threshold |
| --- | --- | --- |
| Overhangs | Color map of downward faces steeper than the limit, for the current print orientation | 45° from vertical (FDM) |
| Thin walls | Heatmap of wall thickness | Below 2 × nozzle diameter |
| Small features | Holes, pins and text too small to print cleanly | Below 1 × nozzle (holes below 2 mm flagged) |
| Bridges | Unsupported horizontal spans | Above profile bridge length |
| Bed fit | Part outline against the bed; suggests a split plane when it doesn't fit | Profile bed size |
| Islands | Regions that start mid-air in the chosen orientation | Any |
| First-layer contact | Contact area with the bed; warns on tall, thin parts | Below 5% of the footprint |
| Trapped volumes | Enclosed hollow pockets (resin) | Any, with drain hole suggestion |
| Clearance | Gaps between bodies or moving joints below the fit class | Profile slip fit |
| Mesh validity | Watertight, manifold output before export | Always on export |

Checks run incrementally after each recompute, on a background thread, and never block modeling. A single "Print health" badge in the status bar sums them up; clicking it opens the list, and clicking an item zooms to the problem.

**Print-aware features**

- **Holes:** teardrop or flat-top shapes for horizontal holes, sacrificial bridging layers for counterbores printed upside down, and heat-set insert presets (M2–M5, common brands).
- **Threads:** FDM-tuned thread profiles (enlarged clearance, truncated crests), modeled threads by default, and nut and bolt pockets with captive-nut presets.
- **Edges:** elephant's-foot chamfer on bed-contact edges, and "fillet only vertical edges" or "chamfer only bottom edges" quick selections.
- **Joinery:** split-with-connectors (dowel pins, dovetails, alignment keys) for parts bigger than the bed; snap-fit cantilever feature; living hinge; print-in-place hinge generator.
- **Text and labels:** emboss and deboss presets with minimum stroke checks.
- **Orientation:** auto-orient (minimize support area, maximize bed contact, or favor surface finish on a chosen face) and a manual "lay face on bed" tool.

**Slicer handoff**

- **Desktop:** detects installed Bambu Studio, OrcaSlicer, PrusaSlicer and Cura. "Send to slicer" writes a 3MF to a temp folder and opens it in the chosen slicer.
- **Watch mode:** the exported 3MF is rewritten on every save, so slicers that support reload pick up changes without re-importing.
- **Web:** downloads the 3MF, or opens it through slicer URL schemes where the slicer supports them.

**Multi-material and color**

- Each body carries color, material and extruder/filament slot. The 3MF export uses the Materials and Properties extension, plus Bambu/Orca and Prusa project metadata, so filament slots arrive already assigned.
- Bodies can be marked as **modifiers**: negative volume, support blocker, support enforcer, or settings modifier (for example, 100% infill around a screw boss). They export as slicer modifier volumes, not printed parts.
- Painted color on faces (for multi-color printing on a single body) is planned for v1.1.

**Resin (MSLA/SLA) in v1**

Resin reuses most of the print module (profiles, checks, orientation and export) and adds roughly 15–20% to its scope.

- **Hollowing:** an inward shell that keeps the outer surface, with wall thickness from the profile. Internal lattice infill comes in v1.1.
- **Drain and vent holes:** placed on hollowed bodies, with matching plugs generated as separate bodies for sealing after printing.
- **Suction-cup check:** finds hollow or concave regions that open toward the build plate and would hold a vacuum during peel, for the chosen orientation.
- **Layer-island check:** slices the model at layer height and flags islands that start unsupported. Generating supports stays the resin slicer's job.
- **Resin-aware orientation:** auto-orient favors tilted placements that cut peel force and per-layer cross-section area.
- **Resin fit classes:** tighter default clearances than FDM, tuned with a resin calibration print.
- **Slicer handoff:** Lychee Slicer, Chitubox and PrusaSlicer (SLA mode), through STL, 3MF or OBJ export.

## 12. Import and export

STEP is the exchange format for exact geometry, 3MF is the main print export, and every other format is a convenience. Only `.ptsm` and `.py` reopen as editable features.

| Format | Import | Export | Notes |
| --- | --- | --- | --- |
| `.ptsm` / `.ptsz` | Yes | Yes | Native, fully parametric |
| `.py` (our API) | Yes | Yes | Code-first models; export from any GUI model |
| STEP (AP203/214/242) | Yes | Yes | Exact B-rep, colors and names; the route for Fusion and Onshape models |
| 3MF | Yes (as mesh) | Yes | Main print export: units, colors, materials, modifiers, multi-plate, slicer metadata |
| STL (binary/ASCII) | Yes (as mesh) | Yes | Per-body or merged; deviation and angle tolerance settings |
| OBJ | Yes (as mesh) | Yes | With colors where supported |
| `.scad` | Yes | No | Through the Rust interpreter (section 10) |
| `.FCStd` | Yes (best effort) | No | Sketches and common Part Design features become live features; the rest become dead solids |
| DXF / SVG | Yes (into sketch) | Yes (from sketch or face) | For laser-cut templates and logos |
| PNG / JPG | Yes (sketch canvas) | Yes (screenshot) |  |
| IGES, BREP | Yes | Yes | Low priority; OCCT supports them almost for free |
| `.f3d` | No | No | Closed format; use STEP |

**Mesh export quality:** tessellation tolerance follows the printer profile (by default, chord deviation of a quarter of the layer height and 5° angular tolerance), so curved surfaces are smooth at print resolution without bloated files.

**Imported geometry** lands as a body with an "Imported" base feature. You can build parametric features on top of it, and replace the source file later while keeping downstream features (references resolve by geometric fingerprint).

## 13. UI and UX design

The viewport fills the screen, and everything else appears only when needed. There are three contexts (Model, Sketch and Print) instead of workbenches. Each context shows one short toolbar of about 10 tools, and every other command is one keystroke away in the command palette. A wireframe follows the layout regions below.

**Principles**

1. **The model is the interface.** Edit by double-clicking geometry or timeline items, drag on-canvas handles, and type values in place. Panels serve the canvas, not the reverse.
2. **One toolbar per context, no ribbons.** The toolbar shows the roughly 10 most-used tools for that context. The rest live in the command palette and the right-click marking menu. Users can pin their own tools.
3. **No modal dialogs.** Feature options appear in a small floating inspector next to the selection. It shows the 2–4 essential options, and a "More" toggle reveals the rest. Enter confirms, Esc cancels, and the preview is always live.
4. **Progressive disclosure.** Beginners see a clean, approachable surface. FreeCAD-level power (attachment modes, solver options, expression editor) is there, but only when asked for. There is one interface for everyone: no beginner or expert modes, and the same screens that a newcomer finds approachable carry the full power an experienced user needs.
5. **Errors explain and offer a fix.** Every failure names the cause in plain words and offers a fix where possible ("Fillet too large: max 2.4 mm. Use 2.4 mm?").
6. **Everything is undoable,** including view changes in sketch mode, and undo history survives a save.

**Layout regions**

- **Top bar:** file name and save state, context switcher (Model / Sketch / Print), the context toolbar, command palette button, share/export.
- **Left (collapsible):** browser of components, bodies, sketches, datums and parameters, with eye, lock and color controls. Hidden by default on small screens.
- **Center:** viewport with a small view cube, a section-view toggle, and the selection filter.
- **Bottom:** the feature timeline (Fusion style) with a rollback marker, warning icons and drag-to-reorder. It collapses to a thin strip.
- **Floating:** the feature inspector, shown only while a command is active or an item is selected.
- **Status bar:** units, printer profile, print-health badge, sketch DOF count, recompute time.

*(Wireframe in the original document: Model context, Shell command active.)*

The inspector appears only while a command runs, next to the selected face. Closing it returns the screen to viewport, browser and timeline.

**Input**

- Command palette (Ctrl/Cmd+K): fuzzy search over every command, parameter, body and setting, showing each command's shortcut.
- Right-click marking menu (radial, Fusion style) with context-aware commands.
- Mouse navigation presets: Fusion, FreeCAD, SolidWorks, Blender, Onshape. Trackpad gestures on laptops. 3Dconnexion SpaceMouse on desktop.
- Shortcut presets matching Fusion and FreeCAD, so switchers keep their muscle memory.

**Visual design**

- Neutral gray UI, a single accent color, and generous spacing. Icons are a consistent monochrome line set; color is reserved for meaning (selection, errors, analysis).
- Light and dark themes from one token set. 1× to 3× UI scaling. Text meets WCAG AA contrast.
- Edges are drawn with screen-space anti-aliased lines, faces use subtle matcap-style shading, and sketches render as crisp vector overlays.

**Onboarding**

- Five-minute first-run tutorial that models a printable part (a parametric box with a lid) and sends it to a slicer.
- Hover tooltips with a short looping animation of what the tool does.
- A "Coming from Fusion / FreeCAD" panel mapping familiar names to ours (Pad → Extrude). In M1 the command palette finds commands by their Fusion and FreeCAD names through its tagged keywords and shows the mapping in the result (for example "FreeCAD: Pad" on Extrude, [ui.md](../spec/ui.md) section 2.10); the panel comes later.

## 14. Performance, recompute and rendering

The app never freezes: geometry work runs off the UI thread, recompute is incremental, and previews are cancellable. The budgets below are release gates, measured on a 2022 mid-range laptop and in Chrome on the same machine for the web build.

| Interaction | Budget (desktop) | Budget (web) |
| --- | --- | --- |
| Sketch drag (500 constraints) | 16 ms per frame | 16 ms per frame |
| Feature preview after a value change | 100 ms typical | 250 ms typical |
| Full rebuild, 50-feature part | 1 s | 3 s |
| Open a 50-feature file with warm cache | 300 ms | 1 s |
| Viewport orbit, 2 M triangles | 60 fps | 60 fps |
| Cold start to usable | 2 s | 4 s (core WASM streamed and cached) |

**Recompute engine**

- The feature graph is a DAG. Changing a parameter marks only its dependents dirty; everything upstream is reused from cache.
- Each feature's output (B-rep plus naming map) is cached in memory and in `.ptsm-cache/`, keyed by a hash of its inputs. Reopening a file or undoing an edit can skip rebuilding entirely.
- Independent branches (separate bodies, separate components) recompute in parallel on desktop. On the web, one worker runs the core; multi-worker support is post-v1.
- Previews are cancellable. If the user changes a value again before the preview finishes, the old job is dropped.
- During drags, fillets and other expensive features downstream can show a fast "proxy" preview, then refine after the drag ends.

**Rendering**

- Tessellation runs per face, in the core, and is cached per feature. Only changed faces are re-tessellated.
- Mesh buffers move to the UI as transferable `ArrayBuffer`s (web) or shared memory (desktop), with no copying of geometry.
- GPU picking by ID buffer gives face, edge and vertex selection in under 1 ms.
- Analysis overlays (overhang map, wall thickness) render as per-vertex attributes, so toggling them costs nothing.

**Memory:** desktop target under 1.5 GB for a 300-feature assembly. Web stays under the 4 GB wasm32 limit, with cache eviction for off-screen components.

## 15. Extensibility and post-v1 readiness

Cloud, AI, plugins and the model library are all deferred, but each one plugs into a seam that v1 already ships and uses internally. No post-v1 feature should need the core restructured.

| Post-v1 feature | Seam built in v1 | What v1 must guarantee |
| --- | --- | --- |
| Plugin / extension API | Feature registry: built-in features register through the same trait (`Feature`) and manifest a plugin will use. UI panels register through a slot API. | Built-in features use no private shortcuts. The feature trait and protocol schema are versioned from day one. |
| AI assist | The Python API and the command protocol. Every user action is a typed, serializable command. | Commands are complete and deterministic, so an AI agent can read the model as text, propose commands or code, and show them as a reviewable preview. |
| Cloud sync and sharing | Storage abstraction (`DocumentStore`) with local-file and IndexedDB backends. Content-hashed assets. | Documents are self-contained folders with stable IDs and no absolute paths, so a sync backend is just another store. |
| Model library and remix | Linked components, `param()` Customizer panel, `.ptsz` bundles. | A shared model opens with its parameters exposed, and a remix keeps a link to its source. |
| Real-time collaboration | Command log as the undo model; record-level file format. | Operations carry stable IDs, so a later CRDT or operational-transform layer can merge them. |

**Plugin model (planned)**

- Python plugins first: they can add features, selectors, importers/exporters, printability checks and panels. They run sandboxed with declared permissions.
- WASM plugins later, for performance-critical features in Rust or C++, loaded identically on desktop and web.
- Plugins declare a version range of the feature API. Files that use a plugin feature record the plugin ID and version, and open with a cached dead solid plus a clear message if the plugin is missing.

**Codebase hygiene that keeps these doors open**

- Cargo workspace with separate crates: `partsmith-model`, `partsmith-recompute`, `partsmith-naming`, `partsmith-features`, `partsmith-sketch`, `partsmith-print`, `partsmith-io`, `partsmith-occt-sys` (the C++ shim), `partsmith-py`, `partsmith-protocol`, plus the TypeScript `ui` package.
- The UI depends only on `partsmith-protocol`, never on core internals.
- No global state in the core; every API takes a document handle. This allows several documents and, later, server-side headless use (`partsmith` CLI for batch export and CI).

## 16. Testing and quality

The core risk in CAD is silent geometric breakage, so testing centers on a large corpus of real models rebuilt on every change. Many small unit tests check each piece; the corpus and the feature examples check whole models. All three gate every merge and release.

- **Unit tests:** simple and plentiful: one behavior per test, no hidden fixtures, fast enough to run on every save. Core crates target at least 90% line and branch coverage, measured with `cargo-llvm-cov` in CI.
- **Feature example suite:** `examples/` holds at least one small `.ptsm` model and its Python twin for every feature and option: each sketch tool and constraint, each feature mode, each import and export path, each printability check, FDM and resin. Each example has expected results (volume, face count, check findings). A CI check fails if any registered feature has no example. The examples double as documentation and tutorial material.

* **Model corpus:** 500+ `.ptsm` models at launch: tutorials, imported FreeCAD files, community models and adversarial cases. CI rebuilds all of them on every merge and compares volume, surface area, bounding box and face count against stored baselines.
* **Topological naming tests:** scripted edits (change a sketch dimension, add a feature upstream, reorder) applied across the corpus. The metric is the share of downstream references that still resolve to the correct element; the v1 target is above 99%.
* **Round-trip tests:** GUI → Python → `.ptsm` → rebuild gives identical geometry. Save → load → save gives byte-identical files. STEP export → import keeps volume within 1e-6 relative error.
* **Fuzzing:** `cargo fuzz` on the file parser, the `.scad` interpreter and the protocol decoder. Property-based tests (proptest) on the sketch solver with random constraint sets.
* **Performance regression:** the budgets in section 14 run in CI on fixed hardware runners; a regression over 10% blocks the merge.
* **Visual tests:** screenshot comparisons of the viewport and UI states in light and dark themes, on desktop and web.
* **Print validation:** a monthly physical print of calibration and regression parts (fit classes, threads, teardrop holes) on at least one FDM and one resin printer.
* **Kernel upgrades:** an OCCT version bump runs the full corpus and needs explicit sign-off on any changed baseline.

## 17. Development with AI agents

Claude Code agents write all of Partsmith's code. Humans set direction, approve the decisions that are expensive to reverse, and judge how the app feels. That works only if the specs and tests, not the agents' own judgment, decide what "done" means, so this section is as much a part of the architecture as the crates.

**Repository set-up for agents**

- One monorepo: the Cargo workspace, the TypeScript UI, the Python package, the docs and the model corpus.
- A root `AGENTS.md` holds build and test commands, coding rules, the engineering principles from section 1, and the invariants that must never break (deterministic serialization, no index-based references, no C++ types outside `partsmith-occt-sys`). Each crate has its own short `AGENTS.md` with its purpose, public interface and test commands.
- This design doc becomes `docs/design/`. Every lasting decision is an ADR in `docs/adr/`. Agents read the relevant ADRs before changing an area and write a new ADR when they change a decision.
- `docs/spec/<feature>.md` describes each feature's behavior, parameters, edge cases and acceptance tests before any code is written.
- Small crates with narrow, documented interfaces, so one agent can hold the whole area it is working on in context.

**Agent roles**

| Role | Does | Never does |
| --- | --- | --- |
| Planner | Breaks each milestone into issues from this doc and the specs; each issue has acceptance tests and touches one crate where possible | Writes production code |
| Test author | Turns a spec into failing tests and corpus cases before implementation starts | Sees the implementation while writing tests |
| Implementer | Works one issue in its own git worktree until the tests pass; several run in parallel on different crates | Edits tests, the file-format schema or the protocol schema |
| Reviewer | Fresh context; checks the pull request against the spec, ADRs and invariants; runs the full corpus; looks at screenshots of UI changes | Reviews its own work |
| Maintainer | Triages CI failures, flaky tests and corpus regressions; files issues | Merges without a green build |

Splitting test authoring from implementation keeps an agent from grading its own work. A **held-out corpus** of models that implementers never see catches code that passes the visible tests without being correct.

*(Diagram in the original document: agent task flow, one issue from spec to merge.)*

A failed check or a review comment sends the issue back to the implementer; only schema and license changes wait for a human.

**Test oracles agents can trust**

- **Differential testing:** each corpus model also exists as an equivalent build123d script. CI builds both and compares volume, surface area, bounding box and topology. Disagreement points to a bug in one of them, found without a human.
- **Geometric invariants:** watertight output, no self-intersections, and volume changes consistent with the feature (a cut never adds volume).
- **Golden files:** canonical `.ptsm` output and 3MF exports compared byte for byte.
- **UI checks:** Playwright drives the web app through scripted tasks, and screenshot diffs catch visual changes for the reviewer to judge.

**Guardrails in CI**

- `rustfmt`, `clippy` with warnings as errors, and `#![forbid(unsafe_code)]` everywhere except the FFI crate.
- `cargo-deny` enforces the license policy for crates: only permissive licenses, and a GPL dependency fails the build, protecting the Apache-2.0 license. OCCT is not a crate, so cargo-deny does not cover it: the notices and published source of delivery.md 2.9 and 2.13 do (ADR 0034).
- `cargo-semver-checks` and schema checks flag any change to the plugin API, protocol or file format.
- No merge without a green build, a passing corpus and reviewer approval.

**Where humans decide**

- Every milestone gate (section 18).
- Changes to the file-format schema, protocol schema, naming algorithm, licenses and dependencies, and anything the design system controls.
- A weekly hands-on review of the latest preview deployment for feel and usability, which agents can't judge well. Findings become issues.
- Releases: a human approves each tagged version before it is published.

## 18. Roadmap and milestones

v1.0 lands through four milestones, each closed by a measurable gate rather than a date. The first task of phase 0 is the Rust + OCCT WebAssembly build spike, because it is the largest technical unknown.

*(Diagram in the original document: roadmap, 4 phases, 4 gates.)*

Configurations, face color painting and resin lattice infill follow in v1.1. Cloud, AI assist, plugins and the model library come after that, through the seams in section 15. Because agents write the code, pace is set by agent compute, CI capacity and how fast human reviewers clear each gate, not by headcount.

## 19. Risks and open questions

The biggest risks are the OCCT-to-WASM build from Rust, topological naming robustness, and the sheer scope of feature parity. Each has a mitigation and an early checkpoint.

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Rust + OCCT on `wasm32-unknown-emscripten` is fragile or slow | High: blocks the web build | Build spike is the first task of phase 0; fallback is a C++ WASM module for OCCT called from a separate Rust WASM module through JS |
| OCCT failures on fillets and booleans in edge cases | High: user trust | Pre-checks, healing, clear errors, and upstream patch contributions; corpus regression tests |
| Topological naming misses | High: broken models after edits | Semantic queries plus fingerprints, repair mode (ghost and candidates), and a dedicated metric in CI (section 16) |
| Parity scope too large for the team | High: late v1 | Parity matrix ranked by real usage; v1.1 holds the long tail (configurations, advanced surfacing) |
| Pyodide + WASM core interop on the web | Medium | Prototype in phase 1; if needed, ship web scripting in v1.1 with code-first files desktop-only at launch |
| WASM memory limit (4 GB) for large models | Medium | Cache eviction; wasm64 (Memory64) when browsers ship it widely |
| Slicer integration breaks when slicers change formats | Medium | Plain 3MF always works; slicer-specific metadata is an optional layer with tests per slicer version |
| Script trust and security | Medium | Trust prompt and sandboxing (section 10) |
| Core WASM modules exceed Cloudflare Pages' 25 MiB per-file limit | Medium: blocks web deploys | Split the core into lazily loaded modules; serve any oversized file from R2 |

**Open questions for you**

- [x] Name and extension: Partsmith, `.ptsm` and `.ptsz`. Run a trademark check before public launch.
- [x] License: Apache-2.0 (ADR 0028).
- [x] Resin in v1: yes.
- [x] Web hosting: Cloudflare Pages, deployed by CI.
- [ ] UI framework: SolidJS (proposed earlier) or React. Agents write React more reliably because far more React code exists to learn from, so React is now the leaning choice.
- [ ] Who are the human reviewers at gates, and how many hours a week can they give (section 17)?
- [ ] Agent compute budget. It sets how many agents run in parallel, and so how fast milestones land.
- [ ] Domain name, and whether the GitHub org and Cloudflare account belong to a project organization or a person.
