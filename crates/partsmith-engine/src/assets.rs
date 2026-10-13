//! Document assets: the content-addressed names records read, and the bytes stored for them
//! (engine.md section 3.14).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_model::Document;

use crate::error::EngineError;
use crate::types::Asset;

/// The asset names the valid records of `document` read.
fn asset_names(registry: &Registry, document: &Document) -> BTreeSet<String> {
    document
        .features
        .iter()
        .filter_map(|record| registry.dependencies(record).ok())
        .flat_map(|dependencies| dependencies.assets)
        .map(|asset| asset.name)
        .collect()
}

/// The supplied assets `document` references, checked against their names (section 3.14).
pub(crate) fn referenced_assets(
    registry: &Registry,
    document: &Document,
    assets: Vec<Asset>,
) -> Result<BTreeMap<String, Arc<[u8]>>, EngineError> {
    let referenced = asset_names(registry, document);
    let mut stored = BTreeMap::new();
    for asset in assets {
        if !referenced.contains(&asset.name) {
            continue;
        }
        if !asset_matches(&asset.name, &asset.bytes) {
            return Err(EngineError::AssetMismatch { name: asset.name });
        }
        stored.insert(asset.name, Arc::from(asset.bytes));
    }
    Ok(stored)
}

/// True when `name` is `<64 lowercase hex>.<ext>` with the BLAKE3 hash of `bytes`.
pub(crate) fn asset_matches(name: &str, bytes: &[u8]) -> bool {
    let Some((hash, extension)) = name.split_once('.') else {
        return false;
    };
    let valid_extension = !extension.is_empty()
        && extension
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    valid_extension && hash == blake3::hash(bytes).to_hex().as_str()
}

/// The content-addressed name of an asset: the BLAKE3 hash of `bytes` and `extension`.
pub(crate) fn asset_name(bytes: &[u8], extension: &str) -> String {
    format!("{}.{extension}", blake3::hash(bytes).to_hex())
}

/// The stored assets `document` references, sorted by name.
pub(crate) fn stored_assets(
    registry: &Registry,
    document: &Document,
    assets: &BTreeMap<String, Arc<[u8]>>,
) -> Vec<Asset> {
    let referenced = asset_names(registry, document);
    assets
        .iter()
        .filter(|(name, _)| referenced.contains(*name))
        .map(|(name, bytes)| Asset {
            name: name.clone(),
            bytes: bytes.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_names_are_the_hash_and_extension() {
        let name = asset_name(b"solid", "step");

        assert!(asset_matches(&name, b"solid"));
        assert!(!asset_matches(&name, b"hello"));
        assert!(!asset_matches(&name.replace(".step", ""), b"solid"));
        assert!(!asset_matches(&name.replace(".step", ".STEP"), b"solid"));
    }
}
