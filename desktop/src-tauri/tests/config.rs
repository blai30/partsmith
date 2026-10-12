//! Acceptance tests for the checked-in configuration (desktop.md section 6, tests 33 to 35): `tauri.conf.json`, the capability file and the icons.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use partsmith_desktop::COMMAND_NAMES;
use serde_json::{Value, json};

/// The crate folder, `desktop/src-tauri`.
fn crate_folder() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Parses a JSON file of the crate folder.
fn json_file(relative: &str) -> Value {
    let path = crate_folder().join(relative);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reading {} failed: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

/// The names of the files in a folder of the crate.
fn file_names(relative: &str) -> BTreeSet<String> {
    let folder: PathBuf = crate_folder().join(relative);
    fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("listing {} failed: {error}", folder.display()))
        .map(|entry| {
            entry
                .expect("the folder entry can be read")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// The object of desktop.md 2.8.
fn expected_configuration() -> Value {
    json!({
        "$schema": "https://schema.tauri.app/config/2",
        "productName": "Partsmith",
        "mainBinaryName": "partsmith-desktop",
        "identifier": "io.github.blai30.partsmith",
        "build": {
            "frontendDist": "../../ui/dist",
            "devUrl": "http://localhost:5173"
        },
        "app": {
            "withGlobalTauri": true,
            "windows": [
                {
                    "label": "main",
                    "title": "Partsmith",
                    "width": 1440,
                    "height": 900,
                    "minWidth": 960,
                    "minHeight": 600,
                    "center": true,
                    "dragDropEnabled": false
                }
            ],
            "security": {
                "csp": {
                    "default-src": "'self'",
                    "script-src": "'self'",
                    "style-src": "'self' https://fonts.googleapis.com",
                    "font-src": "https://fonts.gstatic.com",
                    "img-src": "'self' data:",
                    "connect-src": "ipc: http://ipc.localhost",
                    "worker-src": "'none'",
                    "object-src": "'none'",
                    "base-uri": "'none'",
                    "form-action": "'none'",
                    "frame-ancestors": "'none'"
                },
                "devCsp": {
                    "default-src": "'self'",
                    "script-src": "'self' 'unsafe-inline'",
                    "style-src": "'self' 'unsafe-inline' https://fonts.googleapis.com",
                    "font-src": "https://fonts.gstatic.com",
                    "img-src": "'self' data:",
                    "connect-src": "'self' ipc: http://ipc.localhost ws://localhost:5173",
                    "worker-src": "'none'",
                    "object-src": "'none'",
                    "base-uri": "'none'",
                    "form-action": "'none'",
                    "frame-ancestors": "'none'"
                },
                "freezePrototype": true
            }
        },
        "bundle": {
            "active": true,
            "targets": ["deb", "rpm", "appimage", "nsis", "app", "dmg"],
            "icon": [
                "icons/32x32.png",
                "icons/128x128.png",
                "icons/128x128@2x.png",
                "icons/icon.icns",
                "icons/icon.ico"
            ],
            "category": "GraphicsAndDesign",
            "shortDescription": "Parametric CAD for 3D printing",
            "longDescription": "Partsmith is an open-source parametric CAD application for designing parts for 3D printing.",
            "publisher": "The Partsmith Contributors",
            "copyright": "Copyright the Partsmith contributors",
            "license": "MIT OR Apache-2.0",
            "homepage": "https://github.com/blai30/partsmith",
            "macOS": {
                "minimumSystemVersion": "13.0"
            }
        }
    })
}

/// The object of desktop.md 2.9.
fn expected_capability() -> Value {
    json!({
        "$schema": "../gen/schemas/desktop-schema.json",
        "identifier": "main",
        "description": "The main window may call the engine, ask for files in native dialogs, use the files the user chose, and report unsaved changes.",
        "windows": ["main"],
        "permissions": [
            "allow-call",
            "allow-pick-open-file",
            "allow-pick-save-file",
            "allow-read-file",
            "allow-write-file",
            "allow-set-unsaved"
        ]
    })
}

/// The bundle icon paths listed in `tauri.conf.json`.
fn bundle_icons() -> Vec<String> {
    json_file("tauri.conf.json")["bundle"]["icon"]
        .as_array()
        .expect("bundle.icon is a list")
        .iter()
        .map(|icon| icon.as_str().expect("each icon is a path").to_owned())
        .collect()
}

#[test]
fn t033_the_tauri_configuration_is_exactly_the_one_of_the_spec() {
    assert_eq!(json_file("tauri.conf.json"), expected_configuration());
}

#[test]
fn t034_the_capability_is_exactly_the_one_of_the_spec() {
    assert_eq!(json_file("capabilities/main.json"), expected_capability());
}

#[test]
fn t034_the_capability_allows_each_command_once_in_registration_order() {
    let expected: Vec<Value> = COMMAND_NAMES
        .iter()
        .map(|name| Value::String(format!("allow-{}", name.replace('_', "-"))))
        .collect();

    assert_eq!(
        json_file("capabilities/main.json")["permissions"],
        Value::Array(expected)
    );
}

#[test]
fn t034_the_capability_folder_holds_only_the_main_capability() {
    assert_eq!(
        file_names("capabilities"),
        BTreeSet::from(["main.json".to_owned()])
    );
}

#[test]
fn the_generated_permission_folder_holds_one_file_per_command() {
    // tauri-build writes these files from COMMAND_NAMES on every build but never deletes one, so a renamed command leaves a stale file.
    let expected: BTreeSet<String> = COMMAND_NAMES
        .iter()
        .map(|name| format!("{name}.toml"))
        .collect();

    assert_eq!(file_names("permissions/autogenerated"), expected);
}

#[test]
fn t035_every_bundle_icon_exists() {
    for icon in bundle_icons() {
        assert!(
            crate_folder().join(&icon).is_file(),
            "{icon} exists under desktop/src-tauri"
        );
    }
}

#[test]
fn t035_the_icon_folder_holds_only_the_bundle_icons() {
    let listed: BTreeSet<String> = bundle_icons()
        .iter()
        .map(|icon| {
            icon.strip_prefix("icons/")
                .unwrap_or_else(|| panic!("{icon} is in icons/"))
                .to_owned()
        })
        .collect();

    assert_eq!(file_names("icons"), listed);
}
