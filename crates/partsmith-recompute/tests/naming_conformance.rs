//! Naming conformance of the built-in features (naming.md section 6, tests 81 to 90), run as
//! documents (features.md section 6.1) with the ids naming.md shows. Element names come from the
//! naming maps of the body reports; positions and normals from the kernel's element geometry.
//! Test 196 checks every example against the naming baseline (`feature_support::naming_baseline`).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{fs, thread};

use crate::common;
use crate::feature_support::naming_baseline::{
    RECORD_IN_CI, StoredReference, baseline_files, corpus_tests, files_below, mismatches,
    read_runs, record_mode, recording, run_entry, shown_path, stored_references, write_runs,
};
use crate::feature_support::{Assets, Run, examples_folder, load_example};

use common::{
    Harness, assert_face_normal, assert_face_position, assert_has_element, bodies, document,
    element_names, face, face_count, face_names, final_state, owned, rectangle, rectangle_region,
    sketch, state_before, status,
};
use partsmith_features::BodyState;
use partsmith_model::Document;
use partsmith_naming::{ElementKind, ElementName, MatchMethod, Reference, Resolution, resolve};
use serde_json::{Value, json};

/// An extrude record of one profile region.
fn extrude(feature: &str, sketch: &str, region: Value, extent: Value, op: &str) -> Value {
    json!({
        "id": feature, "type": "extrude", "body": "b-1",
        "profile": {"sketch": sketch, "regions": [region]},
        "extent": extent, "op": op
    })
}

/// The sketch and extrude of naming.md test 81 with the given extent: an 80 x 50 rectangle on XY
/// extruded into b-1.
fn base_model(extent: Value) -> Vec<Value> {
    vec![
        sketch("f-sk1", "XY", rectangle(1, 0.0, 0.0, 80.0, 50.0)),
        extrude(
            "f-ext1",
            "f-sk1",
            rectangle_region(1, 0.0, 0.0, 80.0, 50.0),
            extent,
            "new",
        ),
    ]
}

/// The model of naming.md test 81 (distance 30) followed by `more` features.
fn model_81_then(more: Vec<Value>) -> Document {
    let mut features = base_model(json!({"kind": "distance", "expr": "30"}));
    features.extend(more);
    document(json!({"bodies": bodies(1), "features": features}))
}

/// Strings, sorted.
fn sorted(names: &[&str]) -> Vec<String> {
    let mut names = owned(names);
    names.sort();
    names
}

/// The canonical fingerprint text of the named element of a body state.
fn fingerprint_text(state: &BodyState, name: &str) -> String {
    let elements = state.elements().expect("element infos of the body");
    elements
        .iter()
        .find(|info| info.name.as_str() == name)
        .unwrap_or_else(|| panic!("no element info named {name}"))
        .fingerprint
        .to_string()
}

/// A parsed element name.
fn element(name: &str) -> ElementName {
    ElementName::parse(name).unwrap_or_else(|error| panic!("{name} is not a valid name: {error}"))
}

#[test]
fn t081_extrude_names_laterals_after_their_entities_and_caps_by_role() {
    let mut harness = Harness::builtins();

    let report = harness.run(&model_81_then(Vec::new()));

    let body = final_state(&report, "b-1");
    assert_eq!(
        face_names(body),
        sorted(&[
            "f-ext1/lateral/e-1",
            "f-ext1/lateral/e-2",
            "f-ext1/lateral/e-3",
            "f-ext1/lateral/e-4",
            "f-ext1/start-cap",
            "f-ext1/end-cap"
        ])
    );
    let normals = [
        ("f-ext1/lateral/e-1", [0.0, -1.0, 0.0]),
        ("f-ext1/lateral/e-2", [1.0, 0.0, 0.0]),
        ("f-ext1/lateral/e-3", [0.0, 1.0, 0.0]),
        ("f-ext1/lateral/e-4", [-1.0, 0.0, 0.0]),
        ("f-ext1/start-cap", [0.0, 0.0, -1.0]),
        ("f-ext1/end-cap", [0.0, 0.0, 1.0]),
    ];
    for (name, normal) in normals {
        assert_face_normal(&face(body, name), normal);
    }
    assert_face_position(&face(body, "f-ext1/start-cap"), 2, 0.0);
    assert_face_position(&face(body, "f-ext1/end-cap"), 2, 30.0);
    assert_eq!(
        fingerprint_text(body, "f-ext1/end-cap"),
        "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"
    );
    assert_has_element(body, "edge(f-ext1/end-cap|f-ext1/lateral/e-1)");
    assert_has_element(body, "edge(f-ext1/lateral/e-1|f-ext1/lateral/e-2)");
    assert_has_element(
        body,
        "vertex(f-ext1/end-cap|f-ext1/lateral/e-1|f-ext1/lateral/e-2)",
    );
}

#[test]
fn t082_a_symmetric_extrude_puts_the_start_cap_on_the_far_side() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": base_model(json!({"kind": "symmetric", "expr": "30"}))
    }));

    let report = harness.run(&document);

    let body = final_state(&report, "b-1");
    assert_face_position(&face(body, "f-ext1/start-cap"), 2, -15.0);
    assert_face_normal(&face(body, "f-ext1/start-cap"), [0.0, 0.0, -1.0]);
    assert_face_position(&face(body, "f-ext1/end-cap"), 2, 15.0);
}

#[test]
fn t083_a_through_all_cut_names_its_wall_and_the_edges_it_makes() {
    let mut harness = Harness::builtins();
    let document = model_81_then(vec![
        sketch(
            "f-sk2",
            "XY",
            json!([{"id": "e-5", "type": "circle", "center": [40, 25], "radius": 5}]),
        ),
        extrude(
            "f-ext2",
            "f-sk2",
            json!({"key": "e-5", "fp": "c=40,25|a=78.539816"}),
            json!({"kind": "through_all"}),
            "cut",
        ),
    ]);

    let report = harness.run(&document);

    let body = final_state(&report, "b-1");
    assert_eq!(face_count(body), 7);
    assert_has_element(body, "f-ext2/lateral/e-5");
    assert_has_element(body, "edge(f-ext1/end-cap|f-ext2/lateral/e-5)");
    assert_has_element(body, "edge(f-ext1/start-cap|f-ext2/lateral/e-5)");
    assert_has_element(body, "edge(f-ext2/lateral/e-5)");
}

#[test]
fn t084_a_split_cap_keeps_its_name_with_suffixes_and_old_references_follow_it() {
    let mut harness = Harness::builtins();
    let edge_reference = json!({
        "q": "edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
        "fp": "line|n=1,0,0|c=40,0,30|l=80"
    });
    let document = model_81_then(vec![
        json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "30"}),
        json!({
            "id": "f-sk2", "type": "sketch",
            "plane": {"kind": "datum", "feature": "f-dp1"},
            "entities": rectangle(5, 30.0, -1.0, 50.0, 51.0)
        }),
        json!({
            "id": "f-ext2", "type": "extrude", "body": "b-1",
            "profile": {"sketch": "f-sk2", "regions": [rectangle_region(5, 30.0, -1.0, 50.0, 51.0)]},
            "extent": {"kind": "distance", "expr": "5"}, "reverse": true, "op": "cut"
        }),
        json!({"id": "f-fil1", "type": "fillet", "body": "b-1", "edges": [edge_reference.clone()], "radius": "1"}),
    ]);

    let report = harness.run(&document);

    let slotted = state_before(&report, "f-fil1", "b-1");
    assert_face_position(&face(slotted, "f-ext1/end-cap#1"), 0, 15.0);
    assert_face_position(&face(slotted, "f-ext1/end-cap#2"), 0, 65.0);
    let reference: Reference =
        serde_json::from_value(edge_reference).expect("the edge reference is valid");
    let set = slotted
        .element_set()
        .expect("the slotted body names each element once");
    match resolve(&reference, &set) {
        Resolution::Resolved {
            elements, method, ..
        } => {
            let names: Vec<&str> = elements.iter().map(ElementName::as_str).collect();
            assert_eq!(
                names,
                vec![
                    "edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)",
                    "edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)"
                ]
            );
            assert_eq!(method, MatchMethod::History);
        }
        other => panic!("the old edge reference resolves to {other:?}"),
    }
    assert_eq!(status(&report, "f-fil1"), "ok");
    let filleted = final_state(&report, "b-1");
    assert_has_element(
        filleted,
        "f-fil1/fillet/edge(f-ext1/end-cap#1|f-ext1/lateral/e-1)",
    );
    assert_has_element(
        filleted,
        "f-fil1/fillet/edge(f-ext1/end-cap#2|f-ext1/lateral/e-1)",
    );
}

/// The revolve of naming.md test 85 with this extent.
fn revolve_model(extent: Value) -> Document {
    document(json!({
        "bodies": bodies(1),
        "features": [
            sketch("f-sk1", "XZ", rectangle(1, 10.0, 0.0, 20.0, 10.0)),
            {
                "id": "f-rev1", "type": "revolve", "body": "b-1",
                "profile": {"sketch": "f-sk1", "regions": [rectangle_region(1, 10.0, 0.0, 20.0, 10.0)]},
                "axis": {"kind": "origin", "axis": "Z"}, "extent": extent, "op": "new"
            }
        ]
    }))
}

#[test]
fn t085_a_full_revolve_has_laterals_only_and_a_partial_one_has_caps() {
    let mut harness = Harness::builtins();

    let full = harness.run(&revolve_model(json!({"kind": "full"})));
    let partial = harness.run(&revolve_model(json!({"kind": "angle", "expr": "90 deg"})));

    let laterals = [
        "f-rev1/lateral/e-1",
        "f-rev1/lateral/e-2",
        "f-rev1/lateral/e-3",
        "f-rev1/lateral/e-4",
    ];
    assert_eq!(face_names(final_state(&full, "b-1")), sorted(&laterals));
    let mut with_caps = laterals.to_vec();
    with_caps.extend(["f-rev1/start-cap", "f-rev1/end-cap"]);
    assert_eq!(face_names(final_state(&partial, "b-1")), sorted(&with_caps));
}

#[test]
fn t086_primitives_name_their_faces_by_role() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(5),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"},
            {"id": "f-cyl1", "type": "cylinder", "body": "b-2", "radius": "5", "height": "10", "op": "new"},
            {"id": "f-sph1", "type": "sphere", "body": "b-3", "radius": "10", "op": "new"},
            {"id": "f-cone1", "type": "cone", "body": "b-4", "bottom_radius": "5", "top_radius": "0", "height": "10", "op": "new"},
            {"id": "f-tor1", "type": "torus", "body": "b-5", "major_radius": "20", "minor_radius": "5", "op": "new"}
        ]
    }));

    let report = harness.run(&document);

    let block = final_state(&report, "b-1");
    let front = face(block, "f-box1/front");
    assert_face_normal(&front, [0.0, -1.0, 0.0]);
    for (axis, position) in [0.0, -10.0, 15.0].into_iter().enumerate() {
        assert_face_position(&front, axis, position);
    }
    assert_face_normal(&face(block, "f-box1/right"), [1.0, 0.0, 0.0]);

    let cylinder = final_state(&report, "b-2");
    assert_eq!(
        face_names(cylinder),
        sorted(&["f-cyl1/side", "f-cyl1/top", "f-cyl1/bottom"])
    );
    assert_eq!(
        element_names(cylinder, ElementKind::Edge),
        sorted(&[
            "edge(f-cyl1/side|f-cyl1/top)",
            "edge(f-cyl1/side)",
            "edge(f-cyl1/bottom|f-cyl1/side)"
        ])
    );
    assert_eq!(
        element_names(cylinder, ElementKind::Vertex),
        sorted(&[
            "vertex(f-cyl1/side|f-cyl1/top)",
            "vertex(f-cyl1/bottom|f-cyl1/side)"
        ])
    );

    let sphere = final_state(&report, "b-3");
    assert_eq!(face_names(sphere), owned(&["f-sph1/surface"]));
    assert_eq!(
        element_names(sphere, ElementKind::Edge),
        owned(&["edge(f-sph1/surface)"])
    );
    assert_eq!(
        element_names(sphere, ElementKind::Vertex),
        sorted(&["vertex(f-sph1/surface)#1", "vertex(f-sph1/surface)#2"])
    );

    let cone = final_state(&report, "b-4");
    assert_eq!(
        face_names(cone),
        sorted(&["f-cone1/side", "f-cone1/bottom"])
    );

    let torus = final_state(&report, "b-5");
    assert_eq!(face_names(torus), owned(&["f-tor1/surface"]));
    assert_eq!(
        element_names(torus, ElementKind::Edge),
        sorted(&["edge(f-tor1/surface)#1", "edge(f-tor1/surface)#2"])
    );
    assert_eq!(
        element_names(torus, ElementKind::Vertex),
        owned(&["vertex(f-tor1/surface)"])
    );
}

#[test]
fn t087_a_fillet_face_is_named_after_its_edge_and_other_faces_keep_their_names() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(1),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"},
            {
                "id": "f-fil1", "type": "fillet", "body": "b-1",
                "edges": [{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,30|l=10"}],
                "radius": "2"
            }
        ]
    }));

    let report = harness.run(&document);

    assert_eq!(
        face_names(final_state(&report, "b-1")),
        sorted(&[
            "f-box1/left",
            "f-box1/right",
            "f-box1/front",
            "f-box1/back",
            "f-box1/bottom",
            "f-box1/top",
            "f-fil1/fillet/edge(f-box1/front|f-box1/top)"
        ])
    );
}

#[test]
fn t088_a_shell_names_inner_faces_after_the_kept_faces_and_the_rim_after_the_removed_face() {
    let mut harness = Harness::builtins();
    let document = model_81_then(vec![json!({
        "id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "2",
        "remove_faces": [{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000"}]
    })]);

    let report = harness.run(&document);

    let kept = [
        "f-ext1/lateral/e-1",
        "f-ext1/lateral/e-2",
        "f-ext1/lateral/e-3",
        "f-ext1/lateral/e-4",
        "f-ext1/start-cap",
    ];
    let mut expected: Vec<String> = owned(&kept);
    expected.extend(kept.iter().map(|name| format!("f-sh1/inner/{name}")));
    expected.push("f-sh1/rim/f-ext1/end-cap".to_string());
    expected.sort();
    assert_eq!(face_names(final_state(&report, "b-1")), expected);
}

/// The box of naming.md test 89 and a linear pattern of it along X (3 instances).
fn pattern_model(spacing: &str, into_new_body: bool) -> Document {
    let mut pattern = json!({
        "id": "f-pat1", "type": "linear_pattern", "body": "b-1",
        "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": spacing}
    });
    if into_new_body {
        pattern["op"] = json!("new");
        pattern["target"] = json!("b-2");
    }
    document(json!({
        "bodies": bodies(2),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"},
            pattern
        ]
    }))
}

#[test]
fn t089_pattern_copies_take_instance_suffixes_and_merged_faces_keep_aliases() {
    let mut harness = Harness::builtins();

    let apart = harness.run(&pattern_model("20", true));
    let touching = harness.run(&pattern_model("10", false));

    let copies = final_state(&apart, "b-2");
    assert_has_element(copies, "f-box1/top@f-pat1:1");
    assert_has_element(copies, "f-box1/top@f-pat1:2");
    let merged = final_state(&touching, "b-1");
    assert_has_element(merged, "f-box1/top");
    let aliases = &merged.naming().aliases().0;
    for former in ["f-box1/top@f-pat1:1", "f-box1/top@f-pat1:2"] {
        assert_eq!(
            aliases.get(&element(former)),
            Some(&vec![element("f-box1/top")]),
            "alias of {former}"
        );
    }
}

#[test]
fn t090_a_split_numbers_the_pieces_and_names_the_section_faces_by_side() {
    let mut harness = Harness::builtins();
    let document = document(json!({
        "bodies": bodies(2),
        "features": [
            {"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"},
            {"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "10"},
            {"id": "f-split1", "type": "split_body", "body": "b-1", "plane": {"kind": "datum", "feature": "f-dp1"}, "target": "b-2"}
        ]
    }));

    let report = harness.run(&document);

    let upper = final_state(&report, "b-1");
    let lower = final_state(&report, "b-2");
    assert_face_position(&face(lower, "f-box1/front#1"), 2, 5.0);
    assert_face_position(&face(upper, "f-box1/front#2"), 2, 20.0);
    assert_face_normal(&face(upper, "f-split1/split-positive"), [0.0, 0.0, -1.0]);
    assert_face_normal(&face(lower, "f-split1/split-negative"), [0.0, 0.0, 1.0]);
}

/// A failure of test 196: the file it is about, as failures name it, and its text.
type Problem = (String, String);

/// Every example: each `.ptsm` file under `examples/` outside `assets/` folders, as its path below
/// `examples/` without `.ptsm`, sorted.
fn examples() -> Vec<String> {
    files_below(examples_folder())
        .into_iter()
        .filter(|file| !file.split('/').any(|part| part == "assets"))
        .filter_map(|file| file.strip_suffix(".ptsm").map(str::to_owned))
        .collect()
}

/// The asset bytes of an example: every file in its folder's `assets/`, by name, as `partsmith
/// rebuild` reads them (cli.md section 2.3).
fn example_assets(example: &str) -> Assets {
    let folder = examples_folder().join(example).with_file_name("assets");
    files_below(&folder)
        .into_iter()
        .map(|name| {
            let path = folder.join(&name);
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            (name, Arc::from(bytes))
        })
        .collect()
}

/// Whether a baseline file, relative to the baseline folder, belongs to one of these examples or
/// corpus tests (`<module>::<test>`).
fn has_example_or_test(file: &str, examples: &BTreeSet<&str>, tests: &BTreeSet<String>) -> bool {
    if let Some(example) = file
        .strip_prefix("examples/")
        .and_then(|rest| rest.strip_suffix(".json"))
    {
        return examples.contains(example);
    }
    file.strip_prefix("stability/")
        .and_then(|rest| rest.strip_suffix(".json"))
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(module, test)| tests.contains(&format!("{module}::{test}")))
}

/// The problem of each baseline file whose example or test no longer exists.
fn orphan_problems(examples: &[String]) -> Vec<Problem> {
    let examples: BTreeSet<&str> = examples.iter().map(String::as_str).collect();
    let tests = corpus_tests();
    baseline_files()
        .into_iter()
        .filter(|file| !has_example_or_test(file, &examples, &tests))
        .map(|file| {
            let path = shown_path(&file);
            let text = format!("The naming baseline file {path} has no example or test.");
            (path, text)
        })
        .collect()
}

/// Runs an example once and records its baseline file, or compares the run with the file; an
/// example without a file is not run unless recording.
fn example_problems(example: &str, recording: bool) -> Vec<Problem> {
    let relative = format!("examples/{example}.json");
    let recorded = if recording {
        None
    } else {
        match read_runs(&relative) {
            Some(runs) => Some(runs),
            None => return Vec::new(),
        }
    };
    let document = load_example(&format!("{example}.ptsm"));
    let run = Run::with_assets(document, &example_assets(example));
    let runs = vec![run_entry(&run.document, &run.report)];
    let Some(recorded) = recorded else {
        write_runs(&relative, &runs);
        return Vec::new();
    };
    let path = shown_path(&relative);
    mismatches("", &Value::Array(runs), &Value::Array(recorded))
        .into_iter()
        .map(|mismatch| (path.clone(), format!("{path}: {mismatch}")))
        .collect()
}

/// The problems of every example, run in parallel over `available_parallelism` workers.
fn every_example_problems(examples: &[String], recording: bool) -> Vec<Problem> {
    let next = AtomicUsize::new(0);
    let workers = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut problems = Vec::new();
                    while let Some(example) = examples.get(next.fetch_add(1, Ordering::Relaxed)) {
                        problems.extend(example_problems(example, recording));
                    }
                    problems
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

#[test]
fn t196_m1_names_and_resolutions_match_the_recorded_baseline() {
    let recording = recording();
    let examples = examples();

    let mut problems = orphan_problems(&examples);
    problems.extend(every_example_problems(&examples, recording));

    problems.sort_by(|first, second| first.0.cmp(&second.0));
    let texts: Vec<String> = problems.into_iter().map(|(_, text)| text).collect();
    assert!(texts.is_empty(), "{}", texts.join("\n"));
}

#[test]
fn recording_the_naming_baseline_is_refused_in_ci() {
    let ci = Some(OsStr::new("true"));

    assert_eq!(
        RECORD_IN_CI,
        "PARTSMITH_RECORD_NAMING_BASELINE is set in CI. Record the naming baseline locally and commit it."
    );
    assert_eq!(record_mode(Some(OsStr::new("1")), ci), Err(RECORD_IN_CI));
    assert_eq!(record_mode(Some(OsStr::new("0")), ci), Err(RECORD_IN_CI));
    assert_eq!(record_mode(None, ci), Ok(false));
}

#[test]
fn the_naming_baseline_is_recorded_only_when_its_variable_is_1() {
    assert_eq!(record_mode(Some(OsStr::new("1")), None), Ok(true));
    assert_eq!(record_mode(Some(OsStr::new("0")), None), Ok(false));
    assert_eq!(record_mode(None, None), Ok(false));
}

#[test]
fn baseline_numbers_match_within_the_tolerance_and_integers_exactly() {
    assert!(mismatches("", &json!(1.0000009), &json!(1.0)).is_empty());
    assert!(mismatches("", &json!(1000.0009), &json!(1000.0)).is_empty());
    assert_eq!(
        mismatches("", &json!(1.000002), &json!(1.0)),
        vec![" is 1.000002, expected 1.0"]
    );
    assert_eq!(
        mismatches("/count", &json!(3), &json!(2)),
        vec!["/count is 3, expected 2"]
    );
}

#[test]
fn a_baseline_mismatch_is_reported_at_its_json_pointer() {
    let recorded = json!({"faces": ["f-box1/top", "f-box1/front"], "aliases": {}});
    let fresh = json!({"faces": ["f-box1/top", "f-box1/back"], "direct": {}});

    assert_eq!(
        mismatches("/0", &fresh, &recorded),
        vec![
            r#"/0/faces/1 is "f-box1/back", expected "f-box1/front""#,
            "/0/aliases is missing",
            "/0/direct is unexpected",
        ]
    );
}

#[test]
fn a_stored_reference_takes_the_body_of_the_nearest_object_around_it() {
    let edge = json!({"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"});
    let face = json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,20|a=400"});
    let params = json!({
        "body": "b-1",
        "edges": [edge],
        "plane": {"kind": "face", "body": "b-2", "face": face},
    });

    let found = stored_references(params.as_object().expect("the params are an object"));

    assert_eq!(
        found,
        vec![
            StoredReference {
                pointer: "/edges/0".to_owned(),
                body: Some("b-1"),
                value: &edge,
            },
            StoredReference {
                pointer: "/plane/face".to_owned(),
                body: Some("b-2"),
                value: &face,
            },
        ]
    );
}

#[test]
fn a_baseline_file_belongs_to_an_example_or_a_corpus_test_that_exists() {
    let examples = BTreeSet::from(["box/cube"]);
    let tests = BTreeSet::from(["reference_stability_1::r01_line_redrawn".to_owned()]);

    let belongs = |file: &str| has_example_or_test(file, &examples, &tests);

    assert!(belongs("examples/box/cube.json"));
    assert!(belongs(
        "stability/reference_stability_1/r01_line_redrawn.json"
    ));
    assert!(!belongs("examples/box/gone.json"));
    assert!(!belongs("stability/reference_stability_1/r99_gone.json"));
    assert!(!belongs(
        "stability/reference_stability_9/r01_line_redrawn.json"
    ));
    assert!(!belongs("notes.txt"));
}
