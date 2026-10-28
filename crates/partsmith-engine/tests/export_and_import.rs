//! Acceptance tests 85 to 95, 105, 112, 160 and 161 of `docs/spec/engine.md` section 6: export, STEP import, assets,
//! generated ids and the golden 3MF files.

use crate::common;

use std::ffi::OsStr;
use std::io::{ErrorKind, Read};
use std::path::Path;

use common::{
    GEOMETRY_TOLERANCE, Sent, announce, apply, assert_relative, bless_mode, box_step_bytes,
    doc_handle, fixture, get_document, import_step, measure_body, new_doc, number, open, open_doc,
    open_text, send, send_with_buffers, session, set_rollback,
};
use partsmith_engine::Session;
use partsmith_protocol::Buffer;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// The binary STL format, one file.
fn binary_stl() -> Value {
    json!({"kind": "stl", "binary": true, "per_body": false})
}

/// Sends `export` with a format and no other options.
fn export(session: &mut Session, doc: u64, format: Value) -> Sent {
    send(
        session,
        json!({"type": "export", "doc": doc, "format": format}),
    )
}

/// The `files` of a successful export.
fn files(sent: &Sent) -> Vec<Value> {
    sent.ok()["files"]
        .as_array()
        .expect("export returns files")
        .clone()
}

/// The names of the exported files.
fn file_names(sent: &Sent) -> Vec<String> {
    files(sent)
        .iter()
        .map(|file| file["name"].as_str().expect("files have names").to_owned())
        .collect()
}

/// The text of `3D/3dmodel.model` inside a 3MF package.
fn three_mf_model(bytes: &[u8]) -> String {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a 3MF file is a ZIP package");
    let mut part = archive
        .by_name("3D/3dmodel.model")
        .expect("the package has a model part");
    let mut text = String::new();
    part.read_to_string(&mut text)
        .expect("the model part is UTF-8 text");
    text
}

/// The lowercase hex BLAKE3 hash of `bytes` plus `.step`.
fn asset_name(bytes: &[u8]) -> String {
    format!("{}.step", blake3::hash(bytes).to_hex())
}

/// Exports `box.ptsm` as STEP, imports it into a new document and returns the session, the new document and the import result.
fn imported_box() -> (Session, u64, Vec<u8>, Sent) {
    let mut session = session();
    let bytes = box_step_bytes(&mut session);
    let doc = new_doc(&mut session);
    let imported = import_step(&mut session, doc, &bytes, None);
    (session, doc, bytes, imported)
}

/// Opens a text with one asset passed as request buffer 0.
fn open_with_asset(session: &mut Session, text: &str, name: &str, bytes: &[u8]) -> Sent {
    send_with_buffers(
        session,
        json!({"type": "open_document", "text": text, "assets": [{"name": name, "data": 0}]}),
        vec![Buffer::U8(bytes.to_vec())],
    )
}

#[test]
fn t085_binary_stl_of_the_box_is_one_684_byte_file() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");

    let exported = export(&mut session, doc, binary_stl());

    let files = files(&exported);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "model.stl");
    assert_eq!(files[0]["media_type"], "model/stl");
    assert_eq!(exported.file_bytes(&files[0]).len(), 684);
    assert_eq!(exported.ok()["warnings"], json!([]));
}

#[test]
fn t086_per_body_stl_names_files_after_bodies_in_order() {
    let mut session = session();
    let doc = open_doc(&mut session, "two-bodies.ptsm");

    let exported = export(
        &mut session,
        doc,
        json!({"kind": "stl", "binary": true, "per_body": true}),
    );

    assert_eq!(file_names(&exported), ["Block.stl", "Peg.stl"]);
}

#[test]
fn t087_3mf_has_a_title_only_for_a_single_body() {
    let mut session = session();
    let box_doc = open_doc(&mut session, "box.ptsm");
    let two_doc = open_doc(&mut session, "two-bodies.ptsm");

    let single = export(&mut session, box_doc, json!({"kind": "3mf"}));
    let double = export(&mut session, two_doc, json!({"kind": "3mf"}));

    let single_files = files(&single);
    assert_eq!(single_files.len(), 1);
    assert_eq!(single_files[0]["name"], "model.3mf");
    let single_bytes = single.file_bytes(&single_files[0]);
    assert_eq!(single_bytes[..4], [0x50, 0x4b, 0x03, 0x04]);
    let model = three_mf_model(&single_bytes);
    assert!(
        model
            .lines()
            .any(|line| line == r#" <metadata name="Title">Block</metadata>"#),
        "the model part has the title line:\n{model}"
    );
    let double_model = three_mf_model(&double.file_bytes(&files(&double)[0]));
    assert!(
        !double_model.contains(r#"name="Title""#),
        "no title for two bodies:\n{double_model}"
    );
}

#[test]
fn t088_exports_are_deterministic() {
    let mut session = session();
    let doc = open_doc(&mut session, "box.ptsm");
    let formats = [
        binary_stl(),
        json!({"kind": "3mf"}),
        json!({"kind": "obj"}),
        json!({"kind": "step"}),
    ];

    for format in formats {
        let first = export(&mut session, doc, format.clone());
        let second = export(&mut session, doc, format.clone());
        assert_eq!(file_names(&first), file_names(&second), "{format}");
        let first_bytes: Vec<Vec<u8>> = files(&first)
            .iter()
            .map(|file| first.file_bytes(file))
            .collect();
        let second_bytes: Vec<Vec<u8>> = files(&second)
            .iter()
            .map(|file| second.file_bytes(file))
            .collect();
        assert!(
            first_bytes == second_bytes,
            "{format} exports are byte-identical"
        );
    }
}

#[test]
fn t089_step_round_trip_imports_one_named_body_with_the_same_volume() {
    let (mut session, doc, bytes, imported) = imported_box();

    let result = imported.ok().clone();
    let body = result["body"]
        .as_str()
        .expect("the result names the body")
        .to_owned();
    assert!(
        result["feature"].is_string(),
        "the result names the feature: {result}"
    );
    let document = get_document(&mut session, doc)["document"].clone();
    assert_eq!(document["bodies"].as_array().map(Vec::len), Some(1));
    assert_eq!(document["features"].as_array().map(Vec::len), Some(1));
    assert_eq!(document["bodies"][0]["id"], body.as_str());
    assert_eq!(document["bodies"][0]["name"], "Block");
    let measured = measure_body(&mut session, doc, &body);
    assert_relative(
        number(&measured["volume"]),
        24000.0,
        GEOMETRY_TOLERANCE,
        "volume",
    );

    let named_doc = new_doc(&mut session);
    let named = import_step(&mut session, named_doc, &bytes, Some("Bracket"));
    named.ok();
    let named_document = get_document(&mut session, named_doc)["document"].clone();
    assert_eq!(named_document["bodies"][0]["name"], "Bracket");
}

#[test]
fn t090_saving_returns_the_content_hashed_asset() {
    let (mut session, doc, bytes, imported) = imported_box();
    let body = imported.ok()["body"]
        .as_str()
        .expect("the result names the body")
        .to_owned();
    let volume = number(&measure_body(&mut session, doc, &body)["volume"]);

    let saved = send(&mut session, json!({"type": "save_document", "doc": doc}));

    let assets = saved.ok()["assets"]
        .as_array()
        .expect("save returns assets")
        .clone();
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0]["name"], asset_name(&bytes).as_str());
    assert!(
        saved.u8_buffer(&assets[0]["data"]) == bytes,
        "the asset holds the STEP bytes"
    );
    let text = saved.ok()["text"]
        .as_str()
        .expect("save returns text")
        .to_owned();
    let reopened = open_with_asset(&mut session, &text, &asset_name(&bytes), &bytes);
    let reopened_doc = doc_handle(&reopened);
    let reopened_volume = number(&measure_body(&mut session, reopened_doc, &body)["volume"]);
    assert_relative(reopened_volume, volume, 1e-9, "volume after reopening");
}

#[test]
fn t091_a_missing_asset_fails_the_import_feature() {
    let (mut session, doc, _, imported) = imported_box();
    let feature = imported.ok()["feature"]
        .as_str()
        .expect("the result names the feature")
        .to_owned();
    let text = common::saved_text(&mut session, doc);

    let reopened = open_text(&mut session, &text);

    let status = reopened.feature_status(&feature);
    assert_eq!(status["status"], "error");
    assert!(
        status["messages"]
            .as_array()
            .expect("statuses list messages")
            .iter()
            .any(|message| message["message"]
                .as_str()
                .is_some_and(|text| text.contains("is not in this document's assets folder"))),
        // Features.md section 4.1, `asset_missing`.
        "a message says the asset is missing: {status}"
    );
}

#[test]
fn t092_referenced_assets_must_match_their_hash() {
    let (mut session, doc, bytes, _) = imported_box();
    let text = common::saved_text(&mut session, doc);
    let name = asset_name(&bytes);

    let mismatched = open_with_asset(&mut session, &text, &name, b"hello");
    let unreferenced = open_with_asset(&mut session, &fixture("box.ptsm"), &name, b"hello");

    assert_eq!(mismatched.error_code(), "asset_mismatch");
    unreferenced.ok();
}

#[test]
fn t093_unreadable_step_data_fails_the_import() {
    let mut session = session();
    let doc = new_doc(&mut session);

    let imported = import_step(&mut session, doc, b"hello", None);

    assert_eq!(imported.error_code(), "import_failed");
    assert_eq!(
        imported.error_message(),
        "Import failed. The file is not a valid STEP file."
    );
}

#[test]
fn t094_exports_need_existing_bodies_with_geometry() {
    let mut session = session();
    let doc = new_doc(&mut session);

    let empty = export(&mut session, doc, binary_stl());
    let unknown = send(
        &mut session,
        json!({"type": "export", "doc": doc, "format": binary_stl(), "bodies": ["b-9"]}),
    );

    assert_eq!(empty.error_code(), "nothing_to_export");
    assert_eq!(unknown.error_code(), "unknown_body");
}

#[test]
fn t095_a_rolled_back_body_has_no_geometry_to_export() {
    let mut session = session();
    let doc = open_doc(&mut session, "two-bodies.ptsm");
    apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-cyl1"))]),
    )
    .ok();

    let sent = send(
        &mut session,
        json!({"type": "export", "doc": doc, "format": binary_stl(), "bodies": ["b-2"]}),
    );

    assert_eq!(sent.error_code(), "body_empty");
    assert_eq!(sent.error_message(), r#"Body "Peg" has no geometry yet."#);
}

#[test]
fn t105_sequential_ids_skip_ids_the_document_already_uses() {
    let mut session = session();
    let bytes = box_step_bytes(&mut session);
    let text = r#"{
  "format": "partsmith",
  "format_version": 1,
  "units": "mm",
  "parameters": [],
  "bodies": [],
  "features": [
    {"id": "f-000000000001", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10 mm"}
  ]
}
"#;
    let doc = doc_handle(&open_text(&mut session, text));

    let imported = import_step(&mut session, doc, &bytes, None);

    assert_eq!(imported.ok()["feature"], "f-000000000002");
    assert_eq!(imported.ok()["body"], "b-000000000001");
}

#[test]
fn t112_an_import_under_the_rollback_marker_is_inserted_active() {
    let mut session = session();
    let bytes = box_step_bytes(&mut session);
    let opened = open(&mut session, "two-bodies.ptsm");
    let doc = doc_handle(&opened);
    apply(
        &mut session,
        doc,
        Some("Roll back"),
        json!([set_rollback(Some("f-cyl1"))]),
    )
    .ok();

    let imported = import_step(&mut session, doc, &bytes, None);

    let feature = imported.ok()["feature"]
        .as_str()
        .expect("the result names the feature")
        .to_owned();
    let body = imported.ok()["body"]
        .as_str()
        .expect("the result names the body")
        .to_owned();
    let document = get_document(&mut session, doc)["document"].clone();
    let record = &document["features"][1];
    assert_eq!(record["id"], feature.as_str());
    assert_eq!(record["type"], "import");
    let keys: Vec<&str> = record
        .as_object()
        .expect("records are objects")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["id", "type", "body", "asset", "op"]);
    assert_eq!(record["body"], body.as_str());
    assert_eq!(record["asset"], asset_name(&bytes).as_str());
    assert_eq!(record["op"], "new");
    assert_eq!(document["rollback"], "f-cyl1");
    assert_eq!(imported.feature_status(&feature)["status"], "ok");
    assert_eq!(imported.feature_status("f-cyl1")["status"], "rolled_back");
}

/// Where the golden 3MF files are, as their messages name them (engine.md section 3.20).
const GOLDEN_FOLDER: &str = "crates/partsmith-engine/tests/fixtures";

/// The command that regenerates the golden 3MF files, and only them (engine.md section 3.20).
const BLESS_GOLDENS: &str =
    "PARTSMITH_BLESS=1 cargo test -p partsmith-engine --test integration export_and_import::t160";

/// What test 160 does with a golden 3MF file that it does not fail on (engine.md section 3.20).
#[derive(Debug, PartialEq)]
enum GoldenOutcome {
    /// The golden equals the export.
    Unchanged,
    /// The run blesses a missing or differing golden: the export is written and `line` printed.
    Bless { line: String },
}

/// What test 160 does with the golden file `golden`, whose bytes are `current` (none when it is
/// missing), given the 3MF export `exported` of the fixture `fixture` and whether the run blesses:
/// its outcome, or the text it fails with (engine.md section 3.20).
fn golden_outcome(
    current: Option<&[u8]>,
    exported: &[u8],
    blessing: bool,
    fixture: &str,
    golden: &str,
) -> Result<GoldenOutcome, String> {
    let shown = format!("{GOLDEN_FOLDER}/{golden}");
    match current {
        Some(current) if current == exported => Ok(GoldenOutcome::Unchanged),
        _ if blessing => Ok(GoldenOutcome::Bless {
            line: format!("blessed {shown}"),
        }),
        None => Err(format!("{shown} is missing. Run: {BLESS_GOLDENS}")),
        Some(_) => Err(format!(
            "{shown} differs from the 3MF export of {fixture}. On an x86_64 Linux build other than CI's, the build can make the difference, so check CI first. Regenerate it only for a change that engine.md section 3.20 lists: {BLESS_GOLDENS}"
        )),
    }
}

/// Compares the 3MF export of the fixture `fixture` with the golden file `golden` byte for byte,
/// or writes the golden when the run blesses and it is missing or differs (engine.md section
/// 3.20, test 160).
fn check_golden_3mf(fixture: &str, golden: &str) {
    let blessing = bless_mode(
        std::env::var_os("PARTSMITH_BLESS").as_deref(),
        std::env::var_os("CI").as_deref(),
    )
    .unwrap_or_else(|message| panic!("{message}"));
    let mut session = session();
    let doc = open_doc(&mut session, fixture);
    let exported = export(&mut session, doc, json!({"kind": "3mf"}));
    assert_eq!(file_names(&exported), ["model.3mf"]);
    let bytes = exported.file_bytes(&files(&exported)[0]);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(golden);
    let current = match std::fs::read(&path) {
        Ok(current) => Some(current),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("{GOLDEN_FOLDER}/{golden} is not readable: {error}"),
    };
    match golden_outcome(current.as_deref(), &bytes, blessing, fixture, golden) {
        Ok(GoldenOutcome::Unchanged) => {}
        Ok(GoldenOutcome::Bless { line }) => {
            std::fs::write(&path, &bytes).unwrap_or_else(|error| {
                panic!("{GOLDEN_FOLDER}/{golden} cannot be written: {error}")
            });
            announce(&line);
        }
        Err(message) => panic!("{message}"),
    }
}

#[test]
#[cfg_attr(
    not(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu")),
    ignore = "golden 3MF bytes are pinned on x86_64 Linux (engine.md section 3.20)"
)]
fn t160_the_3mf_export_of_the_box_equals_its_golden_file() {
    check_golden_3mf("box.ptsm", "box.3mf");
}

#[test]
#[cfg_attr(
    not(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu")),
    ignore = "golden 3MF bytes are pinned on x86_64 Linux (engine.md section 3.20)"
)]
fn t160_the_3mf_export_of_two_bodies_equals_its_golden_file() {
    check_golden_3mf("two-bodies.ptsm", "two-bodies.3mf");
}

#[test]
fn t160_a_golden_equal_to_the_export_is_left_alone() {
    for blessing in [false, true] {
        assert_eq!(
            golden_outcome(Some(b"PK"), b"PK", blessing, "box.ptsm", "box.3mf"),
            Ok(GoldenOutcome::Unchanged),
            "blessing: {blessing}"
        );
    }
}

#[test]
fn t160_a_bless_writes_a_missing_or_differing_golden_and_prints_its_line() {
    let blessed = Ok(GoldenOutcome::Bless {
        line: "blessed crates/partsmith-engine/tests/fixtures/box.3mf".to_owned(),
    });

    for current in [None, Some(b"PK".as_slice())] {
        assert_eq!(
            golden_outcome(current, b"PK2", true, "box.ptsm", "box.3mf"),
            blessed,
            "current: {current:?}"
        );
    }
}

#[test]
fn t160_a_missing_golden_fails_with_the_command_that_blesses_it() {
    assert_eq!(
        golden_outcome(None, b"PK", false, "box.ptsm", "box.3mf"),
        Err("crates/partsmith-engine/tests/fixtures/box.3mf is missing. Run: PARTSMITH_BLESS=1 cargo test -p partsmith-engine --test integration export_and_import::t160".to_owned())
    );
}

#[test]
fn t160_a_differing_golden_fails_with_the_build_and_the_listed_changes() {
    assert_eq!(
        golden_outcome(Some(b"PK"), b"PK2", false, "two-bodies.ptsm", "two-bodies.3mf"),
        Err("crates/partsmith-engine/tests/fixtures/two-bodies.3mf differs from the 3MF export of two-bodies.ptsm. On an x86_64 Linux build other than CI's, the build can make the difference, so check CI first. Regenerate it only for a change that engine.md section 3.20 lists: PARTSMITH_BLESS=1 cargo test -p partsmith-engine --test integration export_and_import::t160".to_owned())
    );
}

#[test]
fn t161_a_bless_in_ci_fails_with_the_bless_in_ci_message() {
    for bless in ["1", "0"] {
        assert_eq!(
            bless_mode(Some(OsStr::new(bless)), Some(OsStr::new("true"))),
            Err(
                "PARTSMITH_BLESS is set in CI. Bless expected results locally and commit them."
                    .to_owned()
            ),
            "PARTSMITH_BLESS={bless}"
        );
    }
}

#[test]
fn t161_a_bless_outside_ci_blesses() {
    assert_eq!(bless_mode(Some(OsStr::new("1")), None), Ok(true));
}

#[test]
fn t161_another_value_outside_ci_does_not_bless() {
    assert_eq!(bless_mode(Some(OsStr::new("0")), None), Ok(false));
}

#[test]
fn t161_no_bless_variable_means_no_bless() {
    assert_eq!(bless_mode(None, None), Ok(false));
    assert_eq!(bless_mode(None, Some(OsStr::new("true"))), Ok(false));
}
