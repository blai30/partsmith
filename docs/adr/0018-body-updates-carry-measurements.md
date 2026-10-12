# 0018: Body updates carry the measurements of their shape

Status: Proposed, needs human review

## Context

A body update is a body id, its new `Shape` and `NamingMap`, and the change applied (architecture.md, ADR 0011). Building it, a feature already reads the kernel topology and fingerprints of the new shape (to name it) and, for join, cut and intersect, its volume (for the body rules of features.md section 3.5). The next feature on the body read both again: resolving its references needs the element infos, and its body rules need the volume. On bench50, whose largest body ends with 426 faces, these repeated reads added tens of milliseconds to every feature on the large bodies (features.md open issue 5, recompute.md open issue 4), and hosts that list elements read the topology a third time. The hole feature also measured each tool's overlap with a separate kernel `common` before cutting, because `apply_op` made its own cut and gave the feature no access to the kernel result.

## Decision

- `BodyUpdate` gains `measured: Measurements`, with `topology: Option<Arc<TopologyInfo>>` and `volume: Option<f64>`: what the feature measured on the shape, each value exactly what the body state would compute, `None` when not measured (features.md section 2.7).
- `BodyState` gains `from_update` (the next state with those values already known), `topology` (cached like `elements` and `volume`) and `measurements` (the values computed so far, for an update that keeps the state). Recompute makes every state after a feature with `from_update` (recompute.md section 3.5).
- `apply_op_result` applies the body rules to a kernel boolean the feature made itself; `apply_op` is its kernel call followed by it. Hole reads which tools overlap the body from its cut's history (features/hole.md section 3.3).
- The naming bridge's `name_shape` and `name_by_history` return `Named`, the naming map with the topology it was built from.

## Consequences

A plugin that constructs `BodyUpdate` adds `measured: Measurements::default()`, or passes on what it measured. A feature that carried a wrong value would make later features and hosts see a wrong volume or topology, so the contract requires exact values, and the built-in features carry only values they computed for that very shape. Measurements are derived data: the result cache, the cache keys and saved documents do not change.
