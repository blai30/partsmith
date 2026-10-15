//! Builders for region references and match results (spec sections 6.12 and 6.17 to 6.21).

use partsmith_sketch::{
    MatchMethod, Region, RegionFingerprint, RegionKey, RegionLoop, RegionMatch, RegionRef,
    SketchEntity, Succession, build_profile, match_region,
};

use super::benchmark::entity_ids;
use super::{id, ids};

/// A canonical region key, panicking on a fixture mistake.
pub fn key(text: &str) -> RegionKey {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a canonical region key: {error}"))
}

/// A region fingerprint, panicking on a fixture mistake.
pub fn fingerprint(text: &str) -> RegionFingerprint {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a region fingerprint: {error}"))
}

/// A region loop, panicking on a fixture mistake.
pub fn region_loop(text: &str) -> RegionLoop {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a region loop: {error}"))
}

/// A reference written before file format version 2: a key and a fingerprint, no outline.
pub fn reference(key_text: &str, fingerprint_text: &str) -> RegionRef {
    RegionRef {
        key: key(key_text),
        fp: fingerprint(fingerprint_text),
        outline: None,
    }
}

/// A reference with an outline, as `Region::reference()` writes it.
pub fn reference_with_outline(
    key_text: &str,
    fingerprint_text: &str,
    outline_text: &str,
) -> RegionRef {
    RegionRef {
        outline: Some(region_loop(outline_text)),
        ..reference(key_text, fingerprint_text)
    }
}

/// The same reference without its outline, as written before file format version 2.
pub fn without_outline(region: &RegionRef) -> RegionRef {
    RegionRef {
        outline: None,
        ..region.clone()
    }
}

/// The key text and the outline text of a reference.
pub fn region_ref_texts(region: &RegionRef) -> (String, Option<String>) {
    (
        region.key.to_string(),
        region.outline.as_ref().map(ToString::to_string),
    )
}

/// The regions of these entities' profile.
pub fn regions_of(entities: &[SketchEntity]) -> Vec<Region> {
    build_profile(entities).regions
}

/// The key texts of these regions, in order.
pub fn key_texts(regions: &[Region]) -> Vec<String> {
    regions
        .iter()
        .map(|region| region.key.to_string())
        .collect()
}

/// `match(E, R)` of spec section 6.18: `match_region` on the profile of `entities`, with their ids.
pub fn matched(entities: &[SketchEntity], region: &RegionRef) -> RegionMatch {
    match_region(&regions_of(entities), &entity_ids(entities), region)
}

/// `Resolved { indices, method, successions }`.
pub fn resolved(
    indices: &[usize],
    method: MatchMethod,
    successions: Vec<Succession>,
) -> RegionMatch {
    RegionMatch::Resolved {
        indices: indices.to_vec(),
        method,
        successions,
    }
}

/// `Succession { entity, now }`.
pub fn succession(entity: &str, now: &[&str]) -> Succession {
    Succession {
        entity: id(entity),
        now: ids(now),
    }
}
