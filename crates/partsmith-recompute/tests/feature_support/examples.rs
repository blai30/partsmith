//! The examples of a folder under `examples/` and the check that each is canonical, as
//! `partsmith fmt` writes it (file-format.md sections 3.2 and 3.5).

use partsmith_features::Registry;

use super::{examples_folder, parse_document};

/// The `.ptsm` files in `examples/<folder>/`, as paths relative to `examples/`, sorted.
pub fn example_paths(folder: &str) -> Vec<String> {
    let directory = examples_folder().join(folder);
    let entries = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()));
    let mut paths: Vec<String> = entries
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".ptsm"))
        .map(|name| format!("{folder}/{name}"))
        .collect();
    paths.sort();
    paths
}

/// Asserts that `examples/<folder>/` has examples and that every one of them is canonical.
pub fn assert_examples_canonical(folder: &str) {
    let paths = example_paths(folder);
    assert!(!paths.is_empty(), "examples/{folder} has examples");
    for path in paths {
        assert_canonical_example(&path);
    }
}

/// Asserts that `examples/<relative_path>` is canonical, as `partsmith fmt` writes it (file-format.md
/// sections 3.2 and 3.5): every record validates without unknown keys, and saving the document with
/// the registry's canonical params reproduces the file byte for byte.
pub fn assert_canonical_example(relative_path: &str) {
    let path = examples_folder().join(relative_path);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read example {}: {error}", path.display()));
    let mut document = parse_document(&text);
    let registry = Registry::with_builtins();
    for record in &mut document.features {
        let validated = registry
            .validate_and_canonicalize(record)
            .unwrap_or_else(|error| {
                panic!(
                    "{} in examples/{relative_path} is invalid: {error}",
                    record.id
                )
            });
        assert!(
            validated.unknown_keys.is_empty(),
            "{} in examples/{relative_path} has unknown keys {:?}",
            record.id,
            validated.unknown_keys,
        );
        *record = validated.record;
    }
    let saved = partsmith_model::save(&document)
        .unwrap_or_else(|error| panic!("examples/{relative_path} does not save: {error}"));
    assert_eq!(
        saved, text,
        "examples/{relative_path} is not canonical (partsmith fmt would rewrite it)"
    );
}
