# 0005: Feature registry with generically stored params

Status: Accepted

## Context

The design requires built-in features to register through the same `Feature` trait and manifest a plugin will use, with no private shortcuts (section 15).

## Decision

`partsmith-model` stores each feature as a `FeatureRecord` with an id, a type name, an optional name, a suppressed flag and a generic JSON params map. `partsmith-features` defines the `Feature` trait with a typed `Params` struct per feature, and a `Registry` that validates and canonicalizes params, composes their JSON Schema, extracts dependencies, builds geometry and generates Python. Built-in features register through the public registry API.

## Consequences

Plugin features will be stored, validated and edited exactly like built-in ones. The model crate cannot interpret feature params by itself; loading a document goes through the registry to validate and canonicalize them. The file-format schema and the UI types are generated from the registry, so they stay typed.
