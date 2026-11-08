//! Document assets: the content-addressed names records read, the bytes stored for them, the
//! names the store lacks and the files `add_assets` stores (engine.md section 3.14).

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_model::{Document, Id, is_asset_name};
use partsmith_protocol::{AddedAsset, AssetUse, DocHandle};

use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::session::Session;
use crate::state::feature_label;
use crate::types::{Asset, NewAsset};

/// Each asset name the valid records of `document` read, with the ids of the records that read
/// it in timeline order: suppressed and rolled-back records too (section 3.14).
fn asset_readers(registry: &Registry, document: &Document) -> BTreeMap<String, Vec<Id>> {
    let mut readers: BTreeMap<String, Vec<Id>> = BTreeMap::new();
    for record in &document.features {
        let Ok(dependencies) = registry.dependencies(record) else {
            continue;
        };
        for asset in dependencies.assets {
            readers
                .entry(asset.name)
                .or_default()
                .push(record.id.clone());
        }
    }
    readers
}

/// The supplied assets `document` references, checked against their names (section 3.14).
pub(crate) fn referenced_assets(
    registry: &Registry,
    document: &Document,
    assets: Vec<Asset>,
) -> Result<BTreeMap<String, Arc<[u8]>>, EngineError> {
    let readers = asset_readers(registry, document);
    let mut stored = BTreeMap::new();
    for asset in assets {
        let Some(first) = readers.get(&asset.name).and_then(|ids| ids.first()) else {
            continue;
        };
        if !asset_matches(&asset.name, &asset.bytes) {
            let feature = feature_label(registry, document, first);
            return Err(mismatch(&asset.name, &asset.bytes, &feature));
        }
        stored.insert(asset.name, Arc::from(asset.bytes));
    }
    Ok(stored)
}

/// `asset_mismatch` of the file supplied under `name` with `bytes`, named by `feature`, the label
/// of a record that reads it (section 4).
pub(crate) fn mismatch(name: &str, bytes: &[u8], feature: &str) -> EngineError {
    let file = match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("step") => {
            format!(
                "The STEP file of '{feature}' in the assets folder is not the file it imported."
            )
        }
        Some("ttf" | "otf") => {
            format!("The font file of '{feature}' in the assets folder is not the font it uses.")
        }
        _ => format!("The file of '{feature}' in the assets folder is not the file it uses."),
    };
    EngineError::AssetMismatch {
        name: name.to_owned(),
        hash: blake3::hash(bytes).to_hex().to_string(),
        message: format!("{file} Copy the assets folder that came with the document next to it."),
    }
}

/// True when `name` is an asset's file name (`is_asset_name`) whose hash is the BLAKE3 hash of
/// `bytes`.
pub(crate) fn asset_matches(name: &str, bytes: &[u8]) -> bool {
    is_asset_name(name)
        && name
            .split_once('.')
            .is_some_and(|(hash, _)| hash == blake3::hash(bytes).to_hex().as_str())
}

/// The content-addressed name of an asset: the BLAKE3 hash of `bytes` and `extension`.
pub(crate) fn asset_name(bytes: &[u8], extension: &str) -> String {
    format!("{}.{extension}", blake3::hash(bytes).to_hex())
}

/// The asset names the valid records of `document` read and `store` lacks, sorted by name, each
/// with the records that read it in timeline order (section 3.14).
fn missing_from(
    registry: &Registry,
    document: &Document,
    store: &BTreeMap<String, Arc<[u8]>>,
) -> Vec<AssetUse> {
    asset_readers(registry, document)
        .into_iter()
        .filter(|(name, _)| !store.contains_key(name))
        .map(|(name, features)| AssetUse { name, features })
        .collect()
}

/// The stored assets `document` references, sorted by name.
pub(crate) fn stored_assets(
    registry: &Registry,
    document: &Document,
    assets: &BTreeMap<String, Arc<[u8]>>,
) -> Vec<Asset> {
    let readers = asset_readers(registry, document);
    assets
        .iter()
        .filter(|(name, _)| readers.contains_key(*name))
        .map(|(name, bytes)| Asset {
            name: name.clone(),
            bytes: bytes.to_vec(),
        })
        .collect()
}

impl OpenDocument {
    /// The asset names the valid records of the recipe read and the store lacks, sorted by name,
    /// each with the records that read it (section 3.14). Found again only when the revision or
    /// the store changed: the store only grows while the document is open.
    pub(crate) fn missing_assets(&mut self, registry: &Registry) -> &[AssetUse] {
        let current = (self.revision, self.assets.len());
        if self.missing.as_ref().map(|(at, _)| *at) != Some(current) {
            let uses = missing_from(registry, &self.document, &self.assets);
            self.missing = Some((current, uses));
        }
        self.missing.as_ref().map_or(&[], |(_, uses)| uses)
    }
}

impl Session {
    /// Puts files into the document's asset store and recomputes once when a valid record reads a
    /// name it newly holds (section 3.14). Returns each file's name and whether it was stored, in
    /// the order given.
    pub fn add_assets(
        &mut self,
        doc: DocHandle,
        assets: Vec<NewAsset>,
    ) -> Result<Vec<AddedAsset>, EngineError> {
        let registry = Arc::clone(&self.registry);
        let open = self.open_mut(doc)?;
        let readers = asset_readers(&registry, &open.document);
        let mut read_newly = false;
        let added = assets
            .into_iter()
            .map(|asset| {
                let computed = asset_name(&asset.bytes, asset.kind.extension());
                let name = asset.name.unwrap_or_else(|| computed.clone());
                let stored = name == computed;
                if stored && let Entry::Vacant(entry) = open.assets.entry(computed) {
                    read_newly |= readers.contains_key(entry.key());
                    entry.insert(Arc::from(asset.bytes));
                }
                AddedAsset { name, stored }
            })
            .collect();
        if read_newly {
            self.rerun(doc)?;
        }
        Ok(added)
    }

    /// The asset names the valid records of the recipe read and the store lacks, sorted by name,
    /// each with the features that read it in timeline order (section 3.14).
    pub fn missing_assets(&self, doc: DocHandle) -> Result<Vec<AssetUse>, EngineError> {
        let open = self.open(doc)?;
        Ok(missing_from(&self.registry, &open.document, &open.assets))
    }
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

    #[test]
    fn a_mismatch_names_the_file_by_its_kind() {
        let words = |name: &str| mismatch(name, b"hello", "Text 1").to_string();

        assert!(words("a.ttf").starts_with("The font file of 'Text 1' in the assets folder"));
        assert!(words("a.otf").starts_with("The font file of 'Text 1'"));
        assert_eq!(
            words("a.3mf"),
            "The file of 'Text 1' in the assets folder is not the file it uses. Copy the assets folder that came with the document next to it."
        );
    }
}
