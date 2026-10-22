//! Documents committed for the UI's end-to-end tests are canonical at the current format version (ui.md section 6.2), so a `FORMAT_VERSION` bump that leaves one behind fails here. The CLI checks `examples/` the same way.

use crate::common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{E1, with_format_version};
use partsmith_model::{FILE_EXTENSION, FORMAT_VERSION, load, save};
use pretty_assertions::assert_eq;

/// The Playwright fixtures of ui.md section 6.2.
fn ui_fixtures_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/e2e/fixtures")
}

/// The documents directly in `directory`, sorted by path.
fn documents_in(directory: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{} cannot be listed: {error}", directory.display()));
    let mut documents: Vec<PathBuf> = entries
        .map(|entry| entry.expect("the directory entry can be read").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == FILE_EXTENSION)
        })
        .collect();
    documents.sort();
    documents
}

/// Why `text` differs from what this build saves for it, or `None` when it is the same.
fn stale_reason(text: &str) -> Option<String> {
    let loaded = match load(text) {
        Ok(loaded) => loaded,
        Err(error) => return Some(format!("does not load: {error}")),
    };
    if let Some(version) = loaded.migrated_from {
        return Some(format!(
            "is at format version {version}, not {FORMAT_VERSION}"
        ));
    }
    match save(&loaded.document) {
        Ok(saved) if saved == text => None,
        Ok(_) => Some("is not canonical".to_string()),
        Err(error) => Some(format!("does not save: {error}")),
    }
}

#[test]
fn a_document_at_an_older_version_is_stale() {
    assert_eq!(stale_reason(E1), None);
    assert_eq!(
        stale_reason(&with_format_version(E1, Some("1"))),
        Some(format!("is at format version 1, not {FORMAT_VERSION}"))
    );
}

#[test]
fn ui_fixtures_are_canonical_at_the_current_format_version() {
    let documents = documents_in(&ui_fixtures_directory());
    assert!(!documents.is_empty(), "the UI has fixtures");

    let stale: Vec<String> = documents
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{} cannot be read: {error}", path.display()));
            let name = path.file_name().expect("a listed file has a name");
            stale_reason(&text).map(|reason| format!("{}: {reason}", name.display()))
        })
        .collect();

    assert_eq!(stale, Vec::<String>::new());
}
