# 0008: Short random ids with kind prefixes

Status: Accepted, needs human review

## Context

The design asks for a permanent UUID on every sketch entity, feature and parameter. Ids appear in every reference in the text format, so 36-character UUIDs would make each sketch record long and diffs hard to read.

## Decision

Ids are strings matching `[A-Za-z0-9_-]{1,64}`. Generated ids are a kind prefix (`p-`, `b-`, `f-`, `e-`, `c-`) plus 12 lowercase hex characters from a random 48-bit value, checked against the document for uniqueness at generation. Hand-written ids are allowed. Script runs use a deterministic generator.

## Consequences

Ids stay permanent and unique within a document, and readable in diffs. The chance that two branches generate the same id is negligible (about 4e-9 for a thousand new ids on each side), and the merge driver validates uniqueness anyway.
