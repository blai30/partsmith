# partsmith-naming

Element names, `NamingMap`, references (`{q, fp}`), fingerprints and the resolver.

Spec: `docs/spec/naming.md`. Depends only on `partsmith-model` (for `Id`).

## Public interface

Everything is re-exported at the crate root.

- `ElementKind` (face, edge, vertex) and `GeometryKind` (surface, curve and point kinds; `token()`, `element_kind()`).
- `ElementName`: a validated name (`parse`, `kind`, `split_base`, `is_split_descendant_of`, `is_order_dependent`, `face_terms`, `with_split`, `with_instance`, `generated`, `edge`, `vertex`). Serialized as a JSON string. `Generator` (sketch reference or input element) and `Instance` (pattern or mirror copy number) feed `generated` and `with_instance`. Role words live in `roles`.
- `Fingerprint`: geometry kind, direction, centroid, size, radius and adjacency counts with a canonical text form (`new`, `with_adjacency`, `parse`, `Display`). Serialized as its text.
- `Reference { query, fingerprint }`: serialized as `{"q": ..., "fp": ...}`; `new` checks that the kinds agree, `from_info` references an element as it is now.
- Naming inputs (not serialized): `FaceSource` (`New` or `From` input names) per output face, and `TopologyInfo` with `FaceInfo`, `EdgeInfo`, `VertexInfo` (`validate`, `fingerprint` with adjacency counts).
- `NamingMap`: `build(faces, topology, inherited_aliases)`, lookups `name`, `index`, `count`, `names`, `aliases`, and `with_instance`, `project`, `element_infos`. Serialized for the result cache only. `Aliases` maps former names of merged faces to their current split base (`chase`).
- `ElementInfo`: one element as the resolver sees it (name, kind, fingerprint, adjacent face names).
- `resolve(reference, elements, aliases) -> Resolution` (`Resolved`, `Ambiguous`, `Missing`) with `MatchMethod` and `Candidate`; `score` compares fingerprints; `CONFIDENCE_THRESHOLD` and `AMBIGUITY_MARGIN`.
- `NamingError`: every error, with the messages of spec section 4.

## Modules

`grammar` (name validation), `name`, `kind`, `fingerprint` (`mod.rs` the type, construction and display, `number.rs` the number format, `parse.rs` the text parser), `reference`, `topology`, `map` (building, numbering, instances, projection, element infos), `aliases`, `element_info`, `score`, `resolver`, `roles`, `error`.

## Commands

```sh
cargo test -p partsmith-naming
cargo clippy -p partsmith-naming --all-targets -- -D warnings
cargo fmt -p partsmith-naming
cargo bench -p partsmith-naming --bench budgets   # spec section 5 budgets, fails when one is over
```

## Rules

- Pure Rust, no kernel dependency. Works on `ElementInfo` data, never on shapes.
- Changing the naming algorithm needs an ADR and human review.

## Open spec questions (waiting for an ADR or spec edit)

- Error reasons used beyond naming.md section 4's list: `it ends too early`, `instance numbers are n, n.m or 0.m, with n and m of 1 or more and no leading zeros`, `it is not a face name` (`an edge`, `a vertex`), `field "{field}" is required for {kind}`, `field "{field}" needs {count} values`.
- Instance suffixes go after split suffixes (`X#1@f-p:1`, spec test 13), so the split base of a copied piece is the whole name: alias targets and resolver families do not match in instance copies of split faces. Fixing it changes names (ADR).
- Spec test 39 pairs size 2 with x = 1.0000004, which gives the same order with or without quantization; the test pairs size 1 with it so quantization decides.
