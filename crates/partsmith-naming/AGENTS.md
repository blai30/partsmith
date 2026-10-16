# partsmith-naming

Element names, `NamingMap` with aliases, successions and motions, references (`{q, fp, nb, merged}`), fingerprints, `ElementSet` and the resolver that resolves by meaning.

Spec: `docs/spec/naming.md`. Depends only on `partsmith-model` (for `Id`).

## Public interface

Everything is re-exported at the crate root.

- `ElementKind` (face, edge, vertex) and `GeometryKind` (surface, curve and point kinds; `token()`, `element_kind()`).
- `ElementName`: a validated name (`parse`, `kind`, `split_base`, `piece_free`, `is_split_descendant_of`, `is_order_dependent`, `face_terms`, `with_split`, `with_instance`, `generated`, `edge`, `vertex`, `generators`). Serialized as a JSON string. `Generator` (sketch reference or input element) and `Instance` (pattern or mirror copy number) feed `generated` and `with_instance`. `generators()` lists every feature and sketch generator a name depends on as sorted `GeneratorUse { feature, generator }`. Role words live in `roles`.
- `Fingerprint`: geometry kind, direction, centroid, size, radius and adjacency counts with a canonical text form (`new`, `with_adjacency`, `parse`, `Display`). Serialized as its text.
- `Reference { query, fingerprint, anchors, merged }`: serialized as `{"q", "fp", "nb", "merged"}` (`nb` and `merged` omitted when empty) and validated on deserialization (`InvalidReference`). `Anchors` (part to at most `ANCHOR_LIMIT` piece-free face names) and `Merged` (face name in the query to sorted `MergedName`s: `Face` or `Copies { source, feature }`, written `<source>@<feature>:*`). `new` checks that the kinds agree; `from_element(info, set)` builds the reference of an element as it is now, with anchors and merged names (spec section 3.11); `short` drops anchors and merged names.
- Naming inputs (not serialized): `FaceSource` (`New`, or `From` with `(input map index, name)` pairs) per output face, and `TopologyInfo` with `FaceInfo`, `EdgeInfo`, `VertexInfo` (`validate`, `fingerprint` with adjacency counts).
- `NamingMap`: `build(faces, topology, inherited, added_successions, new_motion)`, lookups `name`, `index`, `count`, `names`, `aliases`, `successions`, `motion`, and `moved`, `with_instance(feature, instance, transform)`, `project`, `element_infos` (edges list their vertices). Serialized for the result cache only (`aliases`, `successions` and `motions` omitted when empty). `Aliases` maps former names of merged faces to sorted lists of the faces that carry them now (`chase`, `is_empty`); `Successions` maps a replaced or extended sketch lateral face to the faces that carry its place now, in profile order (`is_empty`).
- `Motion`: an isometry, row-major matrix then translation, compared bitwise (`IDENTITY`, `new`, `then`, `inverse`, `apply_point`, `apply_direction`, `is_proper`).
- `ElementInfo`: one element as the resolver sees it (name, kind, fingerprint, adjacent face names, and for an edge its vertex names).
- `ElementSet::new(elements, aliases, successions)`: the resolver's view of one body state with every name index built once (`elements`, `names`; `DuplicateName` per kind). `NameSet` is its names-only view, also built from a map (`NameSet::new`): `aliases`, `successions`, `family(part, merged)`, `touches(face, anchor)`.
- `resolve(reference, set) -> Resolution` (`Resolved { elements, method, confidence, follow, via, alternatives }`, `Ambiguous { candidates }`, `Missing { lost, candidates }`) with `MatchMethod` (`Exact`, `History`, `Continued`, `Related`, `Guess`), `Follow` (`Replaced`, `Extended`, `Merged`, `Continued`, `Related`), `Lost { parts, pieces, unmet }` and `Candidate { name, meaning, distance, score }`; `score` compares fingerprints; `CONFIDENCE_THRESHOLD` and `AMBIGUITY_MARGIN`.
- `NamingError`: every error, with the messages of spec section 4.

## Modules

`grammar` (name validation), `name`, `kind`, `shape` (names taken apart: piece-free names, skeletons, terms, generators, `extends`, and `InstanceName`, the one reading of `<source>@<feature>:<instance>`), `fingerprint` (`mod.rs` the type, construction and display, `number.rs` the number format, `parse.rs` the text parser), `reference` (references, anchors, merged names and their validation), `pick` (`Reference::from_element`), `topology`, `motion`, `map` (`mod.rs` lookups, instances, projection and element infos, `build.rs` the build algorithm, `motions.rs` base and own motions, `serial.rs` the cache JSON), `aliases`, `chase` (following aliases and successions with the kinds of history used), `canon` (canonical sets, patterns, how sets meet, families), `name_set` (`NameSet` and its indexes), `element_set` (`ElementSet`: element infos with adjacency and incidence lists, and the edges or vertices whose faces pair with given face sets), `element_info`, `score`, `resolver` (`mod.rs` the public types and `resolve`, `parts.rs` query parts, families and live anchors, `steps.rs` steps 1 to 5, `candidates.rs` candidate order), `fast_hash` (the hasher of the internal name indexes), `roles`, `error`.

## Commands

```sh
cargo test -p partsmith-naming
cargo clippy -p partsmith-naming --all-targets -- -D warnings
cargo fmt -p partsmith-naming
cargo bench -p partsmith-naming --bench budgets          # spec section 5 budgets, fails when one is over
cargo bench -p partsmith-naming --bench budgets -- parse # only the operations whose names contain a word
```

## Rules

- Pure Rust, no kernel dependency. Works on `ElementInfo` data, never on shapes.
- Changing the naming algorithm needs an ADR and human review.

## Open spec questions (waiting for an ADR or spec edit)

- Instance suffixes go after split suffixes (`X#1@f-p:1`, spec test 13), so `ElementName::split_base` of a copied piece is the whole name. The resolver does not depend on it: piece-free names (section 3.1) drop every `#n` group, also before `@` groups, so families match in instance copies of split faces.
