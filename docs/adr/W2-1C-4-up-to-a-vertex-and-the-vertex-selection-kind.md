# W2-1C-4: Up to a vertex and the vertex selection kind

Status: Accepted (owner's rule of 2026-11-05); needs human review at the M2 gate: it adds a value to the protocol schema's `SelectionKind` (additive, `PROTOCOL_VERSION` stays 1, m2-plan.md section 7.3 choice 2) and it amends [ADR 0066](0066-sketch-entities-of-format-version-3-operations-and-display-polylines.md) decision 9, a decision of wave 1's spec round. It settles how `up_to_vertex` ends and when the new selection kinds land.

## Context

- **Item 17** names "up to a face, body or vertex". Fusion's "To Object" takes a vertex and ends the extrude on the plane through it, normal to the extrude direction.
- **Vertices already resolve.** `BuildContext::resolve` takes `ElementKind::Vertex` (features.md section 2.6), and a vertex is named by its faces (naming.md section 3.3 step 5). No field picks one: `SelectionKind` has `body`, `sketch`, `sketch_region`, `sketch_point`, `face`, `edge`, `plane` and `axis` (model.md section 2.12).
- **ADR 0066 decision 9** adds `sketch_chain`, `sketch_text`, `path`, `rail` and `section`, and leaves each one's value and UI rule to the spec of the first feature that uses it. model.md section 2.12 adds that they "land together, with the first item that uses one (the sweep's `path`, M2 item 18, wave 2)", and model.md tests 113 and 114 pin it, with 13 values.
- **Feature track order.** The wave 2 decisions put the feature track's code after `PlanarCurve` in the order extents, threads, tapped holes, sweep, coil. Item 17 now lands before item 18.

## Decision

1. **Up to a vertex is a distance.** `up_to_vertex` ends on the plane through the vertex, normal to the extrude direction. Its length is the vertex's signed position along the direction, less the start's. The tool is the plain prism (or the drafted one, with a taper) over that interval, and every profile a distance takes works. No split runs.
2. **Where the vertex lies.** A length of at least `LINEAR_TOLERANCE` builds. A vertex behind the start fails with `up_to_misses` and Turn on Reverse, which always works for a vertex. A vertex within `LINEAR_TOLERANCE` of the start plane fails with `up_to_misses`, whose message says the extrude would have no length. A reference that resolves to several vertices is used when they all lie within `LINEAR_TOLERANCE` of one plane normal to the direction, and otherwise fails with `up_to_ambiguous`, which offers each vertex as a fix.
3. **What a vertex is.** A vertex of a body, stored as a naming `Reference` beside its `body` ([ADR W2-1C-1](W2-1C-1-extent-records-of-extrude-and-revolve.md) decision 1). Sketch points and datum points are later work: a sketch point would need a second record shape, and datum points come with M3's datums.
4. **The selection kind.** `SelectionKind` gains `vertex` ("A vertex of a body"), after `section`. The engine's fix label noun for it is `a vertex` (engine.md section 3.22, already written for a stored vertex reference).
5. **The six kinds land with item 17.** ADR 0066 decision 9 is amended: its five kinds now land in `SelectionKind` with item 17, together with `vertex`, before any feature fills in their value. The spec of the first feature that uses each still gives its value and the UI's rule, as decision 9 says. model.md section 2.12's landing sentence changes to match. `vertex` gets its rule from features/extrude.md and ui.md (track 2C). model.md tests 113 and 114 change by spec first: 14 values, written with item 17.

## Alternatives considered

Every option is open source and adds no dependency. The first criterion of the rule picks none of them. The rule then takes the safest, then the industry standard, then this work's recommendation. Each choice is Accepted (owner's rule of 2026-11-05).

1. *Up to a vertex.* (a) The plane through a body vertex, normal to the direction, with a new `vertex` selection kind; (b) sketch points only, through the existing `sketch_point`; (c) moved to M3 by an ADR. The three are as safe as each other: (a) and (b) build the plain prism of a distance, which needs no spike, and (c) builds nothing. The rule takes (a), the industry standard (Fusion's "To Object") and what item 17 names. It needs no new construction.
2. *When the five kinds of ADR 0066 land.* (a) With item 17, together with `vertex`; (b) `vertex` now and the five with item 18, as decision 9 says. The rule takes (a), the safest: the protocol schema and the UI's exhaustive switches over `SelectionKind` change once, and model.md tests 113 and 114 change once.
3. *Where `vertex` goes in the enum.* (a) After `section`; (b) after `edge`. The rule takes (a), the safest: the M1 values keep their positions in the generated schema's enum.

## Consequences

- model.md section 2.12 gains the value and tests 113 and 114 change by spec first. protocol.schema.json is regenerated with the code.
- protocol.md section 1's list of M2 selection kinds gains `vertex` (stage 1 integrator). ui.md section 3.7 gains a placeholder for `vertex` and its pick rule (track 2C).
- ADR 0066 gets a one-line "Amended by" back-reference at the stage close. Gate 19's review list holds the protocol addition and the amendment.
- The engine's fix label `Pick a vertex again` already exists for a stored vertex reference. engine.md section 3.22's list of selection-kind nouns gains it, and the five kinds' nouns move from item 18 to item 17.
