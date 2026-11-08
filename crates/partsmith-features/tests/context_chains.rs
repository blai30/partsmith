//! Build context tests for curve chains and the region union (features.md section 6.6, tests 32 to 42): chains of the
//! sketch `f-sk2` ('Sketch 2'), built from `PATH` or `ELBOW` of sketch.md section 6.23 by the sketch feature
//! (`common::timeline`), resolved by `BuildContext::chains` for a test feature `f-test1` whose field `path` is labeled
//! Path; the details, codes and fixes of their messages; and the errors `ResolvedProfile::faces` keeps for regions it
//! cannot unite.

use crate::common;

use std::f64::consts::FRAC_PI_2;

use common::records::{arc, edited, line, rectangle, sketch, xy};
use common::samples::{CHAIN_FEATURE, chain_errors, chain_losses, chain_warnings};
use common::timeline::Timeline;
use common::{
    ContextFixture, NoNames, empty_sketch_output, field, hint, id, manifest, pick, pointer,
    render_error, with_standard_labels,
};
use partsmith_features::{
    ChainProfileRef, Diagnosis, FeatureError, FeatureWarning, Fix, Noun, OutputData,
    ResolvedChains, ResolvedProfile, Severity, SketchResult,
};
use partsmith_message_lint::assert_passes_lint;
use partsmith_model::Units;
use partsmith_sketch::{
    ChainKey, ChainLoss, ChainRef, PointRef, Profile, ProfileCurve, ProfileEdge, ProfileLoop,
    Region, SketchEntity, build_chain, build_profile,
};
use serde_json::{Value, json};

/// The test feature the chains are resolved for.
const TEST_FEATURE: &str = "f-test1";

/// The sketch whose chains the test feature uses.
const SKETCH: &str = "f-sk2";

/// The fingerprint of `PATH`'s whole chain (sketch.md test 266).
const PATH_FINGERPRINT: &str = "s=0,0|e=50,40|l=85.707963";

/// `PATH` of sketch.md section 6.23: a line, a quarter arc tangent to it, and a line tangent to the arc.
fn path() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        arc("e-2", [40.0, 10.0], 10.0, -FRAC_PI_2, 0.0),
        line("e-3", [50.0, 10.0], [50.0, 40.0]),
    ]
}

/// `ELBOW` of sketch.md section 6.23 with `e-2` ending at (40, 0.3): its ends are 0.3 mm apart.
fn elbow_with_a_gap() -> Vec<Value> {
    vec![
        line("e-1", [0.0, 0.0], [40.0, 0.0]),
        line("e-2", [40.0, 30.0], [40.0, 0.3]),
    ]
}

/// The sketch `f-sk2`, named 'Sketch 2', on XY with `entities`, built alone.
fn sketched(entities: Vec<Value>) -> Timeline {
    let mut record = sketch(SKETCH, xy(), entities);
    record["name"] = json!("Sketch 2");
    Timeline::run(&[], &[record])
}

/// A stored chain reference from its key and fingerprint texts.
fn chain(key: &str, fingerprint: &str) -> ChainRef {
    serde_json::from_value(json!({"key": key, "fp": fingerprint}))
        .expect("chain references written in tests are valid")
}

/// A chain key, panicking on a fixture mistake.
fn chain_key(text: &str) -> ChainKey {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a chain key: {error}"))
}

/// A point reference of the sketch.
fn sketch_point(text: &str) -> PointRef {
    text.parse()
        .unwrap_or_else(|error| panic!("{text} is a point reference: {error}"))
}

/// A context fixture for `f-test1`, whose manifest labels its field `path` Path, with every output of `timeline`
/// declared as an upstream feature.
fn fixture(timeline: &Timeline) -> ContextFixture {
    let mut fixture = timeline.fixture(TEST_FEATURE, "sketch");
    fixture.manifest = manifest("Test", vec![hint("path", "Path")]);
    fixture
}

/// Resolves `chains` of `f-sk2` at the field `path` of `f-test1` and passes the result to `check`.
fn with_chains<T>(
    timeline: &Timeline,
    chains: Vec<ChainRef>,
    check: impl FnOnce(Result<ResolvedChains<'_>, FeatureError>) -> T,
) -> T {
    let fixture = fixture(timeline);
    let profile = ChainProfileRef {
        sketch: id(SKETCH),
        chains,
    };
    let context = fixture.context();
    check(context.chains("path", &profile))
}

/// The positions of the resolved chains, in order.
fn positions(resolved: &ResolvedChains<'_>) -> Vec<usize> {
    resolved.chains.iter().map(|chain| chain.position).collect()
}

/// `Show` of the sketch, with the entity it names.
fn show(entity: Option<&str>) -> Fix {
    Fix::Show {
        feature: id(SKETCH),
        entity: entity.map(id),
    }
}

/// The message of `warning` on `f-test1`, with the timeline's names, in millimeters.
fn warning_message(timeline: &Timeline, warning: &FeatureWarning) -> String {
    timeline.with_context(TEST_FEATURE, &Diagnosis::default(), |context| {
        warning.render(context)
    })
}

/// The fixes of `warning` on `f-test1`, with the timeline's names.
fn warning_fixes(timeline: &Timeline, warning: &FeatureWarning) -> Vec<Fix> {
    timeline.with_context(TEST_FEATURE, &Diagnosis::default(), |context| {
        warning.fixes(context)
    })
}

/// The message of `error` on `f-test1`, with the timeline's names, in millimeters.
fn error_message(timeline: &Timeline, error: &FeatureError) -> String {
    timeline.with_context(TEST_FEATURE, &Diagnosis::default(), |context| {
        error.render(context)
    })
}

/// The fixes of `error` on `f-test1`, with the timeline's names.
fn error_fixes(timeline: &Timeline, error: &FeatureError) -> Vec<Fix> {
    timeline.with_context(TEST_FEATURE, &Diagnosis::default(), |context| {
        error.fixes(context)
    })
}

/// The one warning of a chain profile that resolved, failing the test otherwise.
#[track_caller]
fn only_warning(resolved: &ResolvedChains<'_>) -> FeatureWarning {
    let [warning] = resolved.warnings.as_slice() else {
        panic!("expected one warning, got {:?}", resolved.warnings);
    };
    warning.clone()
}

/// The error of a chain profile that resolved to nothing, failing the test otherwise.
#[track_caller]
fn chain_error(result: Result<ResolvedChains<'_>, FeatureError>) -> FeatureError {
    match result {
        Err(error) => error,
        Ok(resolved) => panic!(
            "expected chain_not_found, got the chains at {:?}",
            positions(&resolved)
        ),
    }
}

/// The warning of test 33: on `PATH`, `e-1,e-9` at position 1 is lost.
fn lost_second_chain(timeline: &Timeline) -> FeatureWarning {
    with_chains(
        timeline,
        vec![
            chain("e-1,e-2,e-3", PATH_FINGERPRINT),
            chain("e-1,e-9", "s=0,0|e=40,0|l=40"),
        ],
        |result| only_warning(&result.expect("the first chain resolves")),
    )
}

/// The error of test 34: on `ELBOW` with a gap, the only chain is lost.
fn disconnected_elbow(timeline: &Timeline) -> FeatureError {
    with_chains(
        timeline,
        vec![chain("e-1,~e-2", "s=0,0|e=40,30|l=70")],
        chain_error,
    )
}

#[test]
fn t032_one_chain() {
    let timeline = sketched(path());
    let expected = build_chain(
        &timeline.sketch(SKETCH).solved.entities,
        &chain_key("e-1,e-2,e-3"),
    )
    .expect("PATH is one chain");

    with_chains(
        &timeline,
        vec![chain("e-1,e-2,e-3", PATH_FINGERPRINT)],
        |result| {
            let resolved = result.expect("the chain resolves");
            assert_eq!(positions(&resolved), [0]);
            assert_eq!(
                format!("{:?}", resolved.chains[0].chain),
                format!("{expected:?}")
            );
            assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
            assert!(
                resolved.successions.is_empty(),
                "{:?}",
                resolved.successions
            );
        },
    );
}

#[test]
fn t033_a_lost_chain_among_several_is_skipped() {
    let timeline = sketched(path());

    with_chains(
        &timeline,
        vec![
            chain("e-1,e-2,e-3", PATH_FINGERPRINT),
            chain("e-1,e-9", "s=0,0|e=40,0|l=40"),
        ],
        |result| {
            let resolved = result.expect("the first chain resolves");
            assert_eq!(positions(&resolved), [0]);
            let warning = only_warning(&resolved);
            let FeatureWarning::ChainMissing {
                position,
                sketch,
                chain,
                loss,
                ..
            } = &warning
            else {
                panic!("expected chain_missing, got {warning:?}");
            };
            assert_eq!(*position, 1);
            assert_eq!(sketch, &id(SKETCH));
            assert_eq!(chain.key.to_string(), "e-1,e-9");
            assert_eq!(
                loss,
                &ChainLoss::Deleted {
                    entities: vec![id("e-9")],
                    all: false,
                }
            );
            assert_eq!(pointer(&warning).as_deref(), Some("/path/chains/1"));
            assert_eq!(
                warning_message(&timeline, &warning),
                "The curves of 'Sketch 2' selected for Path cannot be used because one of them is no longer in the sketch. They were skipped."
            );
            assert_eq!(
                warning_fixes(&timeline, &warning),
                [show(None), pick("/path/chains/1")]
            );
        },
    );
}

#[test]
fn chains_give_their_warnings_as_messages() {
    let timeline = sketched(path());

    with_chains(
        &timeline,
        vec![
            chain("e-1,e-2,e-3", PATH_FINGERPRINT),
            chain("e-1,e-9", "s=0,0|e=40,0|l=40"),
        ],
        |result| {
            let resolved = result.expect("the first chain resolves");
            let warnings = resolved.warnings.clone();
            assert_eq!(resolved.into_messages(), warnings);
        },
    );
}

#[test]
fn t034_one_chain_none_left() {
    let timeline = sketched(elbow_with_a_gap());

    let error = disconnected_elbow(&timeline);

    let FeatureError::ChainNotFound {
        field,
        sketch,
        chains,
    } = &error
    else {
        panic!("expected chain_not_found, got {error:?}");
    };
    assert_eq!(field.path, "path");
    assert_eq!(field.label, "Path");
    assert_eq!(field.pointer(), "/path");
    assert_eq!(sketch, &id(SKETCH));
    let [lost] = chains.as_slice() else {
        panic!("expected one lost chain, got {chains:?}");
    };
    assert_eq!(lost.position, 0);
    assert_eq!(lost.chain.key.to_string(), "e-1,~e-2");
    let ChainLoss::Disconnected { end, start, gap } = &lost.loss else {
        panic!("expected disconnected ends, got {:?}", lost.loss);
    };
    assert_eq!(*end, sketch_point("e-1.end"));
    assert_eq!(*start, sketch_point("e-2.end"));
    assert!((gap - 0.3).abs() <= 1e-12, "gap {gap}");
    assert_eq!(
        error_message(&timeline, &error),
        "The curves of 'Sketch 2' selected for Path cannot be used because the end of the line from (0, 0) to (40, 0) and the end of the line from (40, 30) to (40, 0.3) are 0.3 mm apart."
    );
    assert_eq!(
        error_fixes(&timeline, &error),
        [
            Fix::ConnectEnds {
                sketch: id(SKETCH),
                first: sketch_point("e-1.end"),
                second: sketch_point("e-2.end"),
            },
            show(Some("e-2")),
            pick("/path/chains/0"),
        ]
    );
}

#[test]
fn t035_several_chains_none_left() {
    let timeline = sketched(path());

    let error = with_chains(
        &timeline,
        vec![
            chain("e-8,e-9", "s=0,0|e=40,0|l=40"),
            chain("e-1,e-9", "s=0,0|e=40,0|l=40"),
        ],
        chain_error,
    );

    let FeatureError::ChainNotFound { chains, .. } = &error else {
        panic!("expected chain_not_found, got {error:?}");
    };
    let lost: Vec<(usize, ChainLoss)> = chains
        .iter()
        .map(|lost| (lost.position, lost.loss.clone()))
        .collect();
    assert_eq!(
        lost,
        [
            (
                0,
                ChainLoss::Deleted {
                    entities: vec![id("e-8"), id("e-9")],
                    all: true,
                }
            ),
            (
                1,
                ChainLoss::Deleted {
                    entities: vec![id("e-9")],
                    all: false,
                }
            ),
        ]
    );
    assert_eq!(
        error_message(&timeline, &error),
        "None of the 2 chains of curves of 'Sketch 2' selected for Path can be used. The curves of the first cannot be used because none of them is in the sketch anymore."
    );
    assert_eq!(
        error_fixes(&timeline, &error),
        [show(None), pick("/path/chains/0")]
    );
}

#[test]
fn t036_equal_keys_use_the_chain_once() {
    let timeline = sketched(path());

    with_chains(
        &timeline,
        vec![
            chain("e-1,e-2,e-3", PATH_FINGERPRINT),
            chain("e-1,e-2,e-3", PATH_FINGERPRINT),
        ],
        |result| {
            let resolved = result.expect("the chain resolves");
            assert_eq!(positions(&resolved), [0]);
            assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        },
    );
}

#[test]
fn equal_keys_use_the_first_entry_that_resolves() {
    let gap = line("e-4", [0.0, 50.0], [0.0, 0.3]);
    let timeline = sketched(edited(rectangle(), &["e-4"], vec![gap]));
    let open = chain("e-1,e-2,e-3,e-4", "s=0,0|e=0,0|l=260");
    let closed = ChainRef {
        closed: true,
        ..open.clone()
    };

    with_chains(&timeline, vec![closed, open], |result| {
        let resolved = result.expect("the open entry resolves");
        assert_eq!(positions(&resolved), [1]);
        let warning = only_warning(&resolved);
        let FeatureWarning::ChainMissing { position, loss, .. } = &warning else {
            panic!("expected chain_missing, got {warning:?}");
        };
        assert_eq!(*position, 0);
        let ChainLoss::Disconnected { end, start, .. } = loss else {
            panic!("expected disconnected ends, got {loss:?}");
        };
        assert_eq!(*end, sketch_point("e-4.end"));
        assert_eq!(*start, sketch_point("e-1.start"));
    });
}

#[test]
fn t037_details() {
    let timeline = sketched(elbow_with_a_gap());
    assert_eq!(
        disconnected_elbow(&timeline).detail().as_deref(),
        Some("path.chains[0] key=e-1,~e-2 disconnected end=e-1.end start=e-2.end gap=0.3")
    );

    let path_timeline = sketched(path());
    assert_eq!(
        lost_second_chain(&path_timeline).detail().as_deref(),
        Some("path.chains[1] key=e-1,e-9 deleted entities=e-9 all=false")
    );
}

#[test]
fn t038_codes_severities_and_repairs() {
    let path_timeline = sketched(path());
    let warning = lost_second_chain(&path_timeline);
    assert_eq!(warning.code(), "chain_missing");
    assert_eq!(warning.severity(), Severity::Warning);
    assert_eq!(warning.repair(), None);

    let elbow_timeline = sketched(elbow_with_a_gap());
    let error = disconnected_elbow(&elbow_timeline);
    assert_eq!(error.code(), "chain_not_found");
    assert_eq!(error.severity(), Severity::Error);
    assert!(error.repairs().is_empty(), "{:?}", error.repairs());
    assert_eq!(error.repair(), None);
}

#[test]
fn t039_an_empty_list() {
    let error = FeatureError::EmptySelection {
        field: field("path", "Path"),
        noun: Noun::Curve,
    };

    assert_eq!(
        render_error(&error, TEST_FEATURE, &NoNames),
        "Select at least one curve for Path."
    );
}

/// `RECT(0, 0, 80, 50)` and the entities drawn after it, as sketch entities.
fn rectangle_with(extra: Vec<Value>) -> Vec<SketchEntity> {
    let mut entities = rectangle();
    entities.extend(extra);
    serde_json::from_value(Value::Array(entities)).expect("entities written in tests are valid")
}

/// The same edge run the other way: its curve reversed and its `reversed` flag flipped.
fn run_backward(edge: &ProfileEdge) -> ProfileEdge {
    let ProfileCurve::Line { start, end } = edge.curve else {
        panic!("the rectangle has only lines, found {:?}", edge.curve);
    };
    ProfileEdge {
        entity: edge.entity.clone(),
        reversed: !edge.reversed,
        curve: ProfileCurve::Line {
            start: end,
            end: start,
        },
    }
}

/// The hand-built region of sketch.md test 300: the rectangle's region with its outer loop run the other way.
fn backward_rectangle() -> Region {
    let region = build_profile(&rectangle_with(Vec::new())).regions[0].clone();
    Region {
        outer: ProfileLoop {
            edges: region.outer.edges.iter().rev().map(run_backward).collect(),
        },
        ..region
    }
}

/// A solved sketch on XY whose profile holds exactly `regions`.
fn sketch_holding(regions: Vec<Region>) -> SketchResult {
    let OutputData::Sketch(mut result) = empty_sketch_output().data else {
        panic!("the empty sketch output holds a sketch result");
    };
    result.profile = Profile {
        regions,
        findings: Vec::new(),
    };
    result
}

/// The `Internal` error that `faces` gives for the regions at `regions` of `sketch`, failing the test otherwise.
#[track_caller]
fn faces_error(sketch: &SketchResult, regions: Vec<usize>) -> FeatureError {
    let profile = ResolvedProfile {
        sketch,
        regions,
        successions: Vec::new(),
        warnings: Vec::new(),
        notices: Vec::new(),
    };
    let Err(error) = profile.faces() else {
        panic!("expected the faces to fail");
    };
    assert_eq!(error.code(), "internal");
    error
}

#[test]
fn t040_the_union_s_errors_keep_their_details() {
    let backward = sketch_holding(vec![backward_rectangle()]);
    assert_eq!(
        faces_error(&backward, vec![0]).detail().as_deref(),
        Some("the selected profile regions do not form a planar arrangement")
    );

    let halves = rectangle_with(vec![line("e-5", [40.0, 0.0], [40.0, 50.0])]);
    let two_regions = sketch_holding(build_profile(&halves).regions);
    assert_eq!(two_regions.profile.regions.len(), 2);
    assert_eq!(
        faces_error(&two_regions, vec![5]).detail().as_deref(),
        Some("the profile has no region 5")
    );
}

#[test]
fn t041_params_json() {
    let text = r#"{"sketch":"f-sk2","chains":[{"key":"e-1,~e-2","fp":"s=0,0|e=40,30|l=70"}]}"#;

    let profile: ChainProfileRef =
        serde_json::from_str(text).expect("a chain profile deserializes");

    assert_eq!(profile.sketch, id(SKETCH));
    assert_eq!(profile.chains, [chain("e-1,~e-2", "s=0,0|e=40,30|l=70")]);
    assert_eq!(
        serde_json::to_string(&profile).expect("a chain profile serializes"),
        text
    );

    let with_outline = r#"{"sketch": "f-sk2", "chains": [{"key": "e-1,~e-2", "fp": "s=0,0|e=40,30|l=70", "outline": "e-1"}]}"#;
    assert!(
        serde_json::from_str::<ChainProfileRef>(with_outline).is_err(),
        "a chain with an unknown member is refused"
    );
}

/// The row of sketch.md section 4.5 a loss renders: its detail's first word, with `all=` for deleted entities and
/// `kind=` for an entity that is not a curve.
fn row(loss: &ChainLoss) -> String {
    let detail = loss.to_string();
    let mut words = detail.split(' ');
    let name = words.next().unwrap_or_default().to_owned();
    let qualifier = words.find(|word| word.starts_with("all=") || word.starts_with("kind="));
    match qualifier {
        Some(qualifier) => format!("{name} {qualifier}"),
        None => name,
    }
}

#[test]
fn t042_the_lint_samples_hold_a_chain_message_for_every_loss() {
    let mut rows: Vec<String> = chain_losses().iter().map(row).collect();
    rows.sort();
    rows.dedup();
    assert_eq!(
        rows,
        [
            "construction",
            "crosses",
            "deleted all=false",
            "deleted all=true",
            "disconnected",
            "not_curve kind=circle",
            "not_curve kind=point",
            "turns_back",
            "zero_length",
        ]
    );
    let warnings = chain_warnings();
    let errors = chain_errors();
    assert_eq!(warnings.len(), chain_losses().len());
    assert!(
        errors.len() > chain_losses().len(),
        "one error of one chain per loss, and one of several chains"
    );

    for units in [Units::Mm, Units::In] {
        for (warning, feature) in &warnings {
            assert_eq!(*feature, CHAIN_FEATURE);
            let text = with_standard_labels(feature, units, &Diagnosis::default(), |context| {
                warning.render(context)
            });
            assert!(
                text.starts_with(
                    "The curves of 'Sketch 4' selected for Path cannot be used because "
                ),
                "{text}"
            );
            assert_passes_lint(&text);
        }
        for (error, feature) in &errors {
            let text = with_standard_labels(feature, units, &Diagnosis::default(), |context| {
                error.render(context)
            });
            assert_passes_lint(&text);
        }
    }
}
