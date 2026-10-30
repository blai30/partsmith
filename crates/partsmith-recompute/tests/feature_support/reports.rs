//! Report readers, shared with `tests/common` (`Run` methods use the same readers): a feature's
//! report, output and body updates, the states of bodies at the end of a run and just before a
//! feature, sketch and datum outputs, rendered messages, dependency links and expression uses, and
//! the region an extrude stores with how it matches its sketch's profile.

use partsmith_features::{
    BodyState, BodyUpdate, DatumResult, Dependencies, FeatureError, FeatureOutput, Link,
    OutputData, SketchResult,
};
use partsmith_model::{FieldUnit, Id};
use partsmith_naming::Motion;
use partsmith_recompute::{BodyReport, FeatureReport, Message, Report};
use partsmith_sketch::{Placement, PointRef, RegionMatch, RegionRef, match_region};

use super::Run;

/// The report of `feature`.
pub fn feature_report<'r>(report: &'r Report, feature: &str) -> &'r FeatureReport {
    report
        .features
        .iter()
        .find(|entry| entry.feature.as_str() == feature)
        .unwrap_or_else(|| panic!("the report has no feature {feature}"))
}

/// The end-of-run report of `body`.
pub fn body_report<'r>(report: &'r Report, body: &str) -> &'r BodyReport {
    report
        .bodies
        .iter()
        .find(|entry| entry.body.as_str() == body)
        .unwrap_or_else(|| panic!("the report has no body {body}"))
}

/// The state of `body` at the end of the run.
pub fn final_state<'r>(report: &'r Report, body: &str) -> &'r BodyState {
    &body_report(report, body).state
}

/// The state of `body` just before `feature` (its `bodies_before`, which holds the bodies it
/// declares).
pub fn state_before<'r>(report: &'r Report, feature: &str, body: &str) -> &'r BodyState {
    feature_report(report, feature)
        .bodies_before
        .iter()
        .find(|(id, _)| id.as_str() == body)
        .map(|(_, state)| state)
        .unwrap_or_else(|| {
            panic!(
                "{feature} has no state of body {body} before it; {}",
                summary(report)
            )
        })
}

/// The output of `feature`; fails the test when it has none.
pub fn output<'r>(report: &'r Report, feature: &str) -> &'r FeatureOutput {
    feature_report(report, feature)
        .output
        .as_deref()
        .unwrap_or_else(|| panic!("{feature} has no output; {}", summary(report)))
}

/// The body updates of `feature`'s output, in output order; fails the test when the output is
/// not body updates.
pub fn body_updates<'r>(report: &'r Report, feature: &str) -> &'r [BodyUpdate] {
    let OutputData::Bodies(updates) = &output(report, feature).data else {
        panic!("{feature} did not output body updates");
    };
    updates
}

/// The `BodyUpdate` of `body` in `feature`'s output.
pub fn output_update<'r>(report: &'r Report, feature: &str, body: &str) -> &'r BodyUpdate {
    body_updates(report, feature)
        .iter()
        .find(|update| update.body.as_str() == body)
        .unwrap_or_else(|| panic!("{feature} did not update body {body}"))
}

/// The placement and motion of the datum plane that `feature` produced.
fn datum_output<'r>(report: &'r Report, feature: &str) -> (&'r Placement, &'r Motion) {
    let OutputData::Datum(DatumResult::Plane { placement, motion }) = &output(report, feature).data
    else {
        panic!("{feature} did not output a datum plane");
    };
    (placement, motion)
}

/// The placement of the datum plane that `feature` produced.
pub fn datum_plane<'r>(report: &'r Report, feature: &str) -> &'r Placement {
    datum_output(report, feature).0
}

/// The rendered messages of a feature's report, error first, then warnings.
pub fn message_texts(report: &FeatureReport) -> Vec<&str> {
    report
        .messages
        .iter()
        .map(|message| message.text.as_str())
        .collect()
}

/// Every feature's status and messages, for failure output.
pub fn summary(report: &Report) -> String {
    let lines: Vec<String> = report
        .features
        .iter()
        .map(|entry| {
            format!(
                "{}: {:?} {:?}",
                entry.feature,
                entry.status,
                message_texts(entry)
            )
        })
        .collect();
    lines.join("; ")
}

/// Asserts that the field an error is about has this path, for example `size[0]`.
pub fn assert_error_field(error: &FeatureError, path: &str) {
    let field = error
        .field()
        .unwrap_or_else(|| panic!("the error names no field: {error}"));
    assert_eq!(field.path, path, "field of the error: {error}");
}

/// The ids of dependency links, in order.
pub fn link_ids(links: &[Link]) -> Vec<&str> {
    links.iter().map(|link| link.id.as_str()).collect()
}

/// The state of `body` just before `feature` (its `bodies_before`, which holds the bodies it declares).
pub fn body_before<'a>(run: &'a Run, feature: &str, body: &str) -> &'a BodyState {
    state_before(&run.report, feature, body)
}

/// The solved sketch that `feature` produced.
pub fn sketch_result<'a>(run: &'a Run, feature: &str) -> &'a SketchResult {
    let OutputData::Sketch(result) = &run.output(feature).data else {
        panic!("{feature} did not output a sketch");
    };
    result
}

/// The solved position of `point` (such as `e-1.end`) in the sketch `feature`.
pub fn solved_point(run: &Run, feature: &str, point: &str) -> [f64; 2] {
    let reference: PointRef = point
        .parse()
        .unwrap_or_else(|error| panic!("{point} is not a point reference: {error}"));
    sketch_result(run, feature)
        .point(&reference)
        .unwrap_or_else(|| panic!("{feature} has no point {point}"))
}

/// The first region the profile of the extrude `extrude` stores.
pub fn stored_region(run: &Run, extrude: &str) -> RegionRef {
    let stored = run.record(extrude).params["profile"]["regions"][0].clone();
    serde_json::from_value(stored)
        .unwrap_or_else(|error| panic!("{extrude} stores a region reference: {error}"))
}

/// How `region` matches the profile of the solved sketch `sketch` (sketch.md section 3.14).
pub fn match_in_sketch(sketch: &SketchResult, region: &RegionRef) -> RegionMatch {
    let entities: Vec<Id> = sketch
        .solved
        .entities
        .iter()
        .map(|entity| entity.id.clone())
        .collect();
    match_region(&sketch.profile.regions, &entities, region)
}

/// How the first region `extrude` stores matches the profile of `sketch` in this run (sketch.md section 3.14).
pub fn stored_region_match(run: &Run, extrude: &str, sketch: &str) -> RegionMatch {
    match_in_sketch(sketch_result(run, sketch), &stored_region(run, extrude))
}

/// The placement of the datum plane that `feature` produced.
pub fn datum_placement<'a>(run: &'a Run, feature: &str) -> &'a Placement {
    datum_plane(&run.report, feature)
}

/// The motion of the datum plane that `feature` produced (features/datum_plane.md section 3.2 step 4).
pub fn datum_motion<'a>(run: &'a Run, feature: &str) -> &'a Motion {
    datum_output(&run.report, feature).1
}

/// The rendered message of `feature` with this code.
pub fn message<'a>(run: &'a Run, feature: &str, code: &str) -> &'a Message {
    run.feature(feature)
        .messages
        .iter()
        .find(|message| message.code == code)
        .unwrap_or_else(|| panic!("{feature} has no {code} message; {}", run.summary()))
}

/// Each expression use of `dependencies` as its field path, expression source and unit, in order.
pub fn expression_uses(dependencies: &Dependencies) -> Vec<(&str, &str, FieldUnit)> {
    dependencies
        .expressions
        .iter()
        .map(|usage| (usage.field.as_str(), usage.expr.as_str(), usage.kind))
        .collect()
}
