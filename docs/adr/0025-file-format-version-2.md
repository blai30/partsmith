# 0025: File format version 2 for anchors, merged names, region outlines and model frames

Status: Accepted, needs human review

## Context

[The reference stability study](../design/reference-stability-and-errors.md) found 35 of 123 corpus tests passing today, and 38 that silently use the wrong element. Its fixes need five pieces of data stored with references, all written by picks and never computed by recompute:

- `nb` (anchors) and `merged` (merged names) on face, edge and vertex references (ADR 0021);
- `outline`, the outer loop in traversal order, on region references (ADR 0022);
- `fallback_frame` on face sketch planes, marking a fallback in body coordinates, and `direction` on edge and face axes (ADR 0023).

`FORMAT_VERSION` and `OLDEST_FORMAT_VERSION` are 1 and `MIGRATIONS` is empty (file-format.md section 3.9). Any change that older readers would misread bumps the version and needs a format ADR and human review. A build that reads only version 1 rejects unknown fields inside a reference (`Reference` uses `deny_unknown_fields`, and sketch records reject unknown nested fields), so a file with `nb` would fail there with a serde "unknown field" error, which [the error guidelines](../guidelines/errors.md) forbid as a message. Saving an unchanged document must stay byte-identical (AGENTS.md invariant), and a file written before the change must keep its meaning.

## Decision

- `FORMAT_VERSION` becomes 2; `OLDEST_FORMAT_VERSION` stays 1.
- Version 2 adds, all optional and omitted when absent: `nb` and `merged` on references (canonical key order `q`, `fp`, `nb`, `merged`), `outline` on region references, `fallback_frame` on face sketch planes (omitted for `world`, the default) and `direction` on edge and face axes. A reference, region or plane without them serializes exactly as in version 1.
- `MIGRATIONS` gets one step from 1 that returns the document unchanged: absent fields keep the version 1 meaning (fallbacks in world coordinates, no anchors, no merged names, no outline), so every version 1 document means the same at version 2, with one exception. An axis without `direction` is oriented by its stored fingerprint direction, which differs from version 1, where the first nonzero component was made positive, only for an axis whose direction crossed a coordinate plane after it was picked (corpus R38: a wall leaning 1 mm the other way); there the old rule flipped the axis, which was the bug. When `load` migrates, the engine tells the user that saving will write version 2 (existing rule).
- One bump covers every addition of the study. The fields are parsed and kept from phase 1, and honored as the phases that use them land.
- Python mirrors the fields as keywords (`Ref(..., anchors=, merged=, direction=)`, `Region(..., outline=)`, `Sketch(..., fallback_frame=)`), written by generated scripts only when stored, so GUI to Python to `.ptsm` keeps every reference's resolution.

## Alternatives considered

- **Add the fields without a version bump.** Older builds would fail with a serde "unknown field" error, or, where unknown keys are tolerated, drop anchors and rebuild differently. A version bump makes them stop with `NewerFormatVersion` and a message that says to update.
- **Rewrite version 1 documents in the migration** (compute anchors, convert fallbacks to body coordinates, fill outlines). A migration works on raw JSON before any recompute and cannot reach the kernel; it would change what saved files mean, and opening and saving an unedited file would change more than its version line. Anchors and outlines are written by the next pick, and fallbacks keep their explicit `world` meaning.
- **One bump per field, as each phase lands.** Three empty migrations and three windows in which new files do not open in slightly older builds, for no benefit: the fields are optional and harmless until used.
- **Keep the new data in a sidecar file or in the result cache.** References must stay self-contained in the one canonical document, and the cache is derived data that may be deleted at any time.

## Consequences

- A version 1 document loads, rebuilds with the same placements, volumes and statuses, and saves at version 2 differing from its input only in the `format_version` line; a version 2 document with every new field round-trips byte for byte. The one exception is an axis whose direction crossed a coordinate plane since it was picked: it now keeps the side it was picked on (file-format.md test 57, with R38's leaning edge). file-format.md tests 23, 39 and E1 to E3 change accordingly (study change FF1).
- Builds that read only version 1 cannot open version 2 files; they show `NewerFormatVersion` ("This file uses format version 2, which is newer than this version of Partsmith supports (up to 1). Update Partsmith to open it.").
- `docs/format/ptsm.schema.json` (the generated `.ptsm` schema, file-format.md section 3.10) and `docs/format/protocol.schema.json` describe the new fields; each check-in is logged in ADR 0016.
- Examples whose references are re-picked gain anchors and outlines and are re-blessed with the sign-off of technical design section 16.
- This changes the file format, so it waits for human review at the next gate. With ADRs 0021 and 0024 it gates all code of the study's plan.
