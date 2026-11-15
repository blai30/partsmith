//! Fuzzes the `.ptsm` loader, `partsmith_model::load_bytes` (delivery.md section 2.15). A
//! document it loads must save, and the saved text must load back to the same document and save
//! to the same text.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // An input the loader refuses passes.
    let Ok(loaded) = partsmith_model::load_bytes(data) else {
        return;
    };
    let text = partsmith_model::save(&loaded.document)
        .unwrap_or_else(|error| panic!("partsmith_model::save(&loaded.document) failed: {error}"));
    let again = partsmith_model::load(&text)
        .unwrap_or_else(|error| panic!("partsmith_model::load(&text) failed: {error}"));
    assert_eq!(
        again.document, loaded.document,
        "partsmith_model::load(&text) returned another document"
    );
    assert_eq!(
        again.migrated_from, None,
        "partsmith_model::load(&text) migrated the saved text"
    );
    let text_again = partsmith_model::save(&again.document)
        .unwrap_or_else(|error| panic!("partsmith_model::save(&again.document) failed: {error}"));
    assert_eq!(
        text_again, text,
        "partsmith_model::save(&again.document) wrote other text"
    );
});
