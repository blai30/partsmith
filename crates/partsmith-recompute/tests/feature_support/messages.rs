//! Helpers for messages, fixes and repairs (features.md sections 3.7, 3.8, 3.12, 4 and 6.5):
//! rendering a feature's errors and warnings with the diagnosis a scenario implies, notices,
//! followed references, picked references, the fixes tests expect, successions, absent faces, the
//! message lint and the choice a reference that no longer lies in one plane or on one line offers.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::{
    BodyState, Diagnosis, DocumentNames, FallbackCause, FeatureError, FeatureOutput,
    FeatureWarning, FieldRef, Fix, Loss, Names, Placed, RenderContext, Severity,
};
use partsmith_message_lint::lint_failures;
use partsmith_model::Id;
use partsmith_naming::{ElementName, Follow, Reference};
use partsmith_recompute::Status;
use serde_json::Value;

use super::{AXIS_TOLERANCE, Run, assert_coordinates, body_before, message};

impl Run {
    /// Calls `use_context` with the render context of `feature` in this run (features.md section 2.8): the document's
    /// names with the run's outputs, the document's units, the states of the feature's declared bodies just before
    /// it, and `diagnosis`. Tests pass the diagnosis a scenario's cause implies (recompute.md section 3.10).
    pub fn with_context<T>(
        &self,
        feature: &str,
        diagnosis: &Diagnosis,
        use_context: impl FnOnce(&RenderContext<'_>) -> T,
    ) -> T {
        let bodies: BTreeMap<Id, BodyState> = self
            .feature(feature)
            .bodies_before
            .iter()
            .cloned()
            .collect();
        let feature = self.feature(feature).feature.clone();
        self.with_names(|names| {
            use_context(&RenderContext {
                feature: &feature,
                names,
                units: self.document.units,
                bodies: &bodies,
                diagnosis,
            })
        })
    }

    /// Calls `use_names` with the document's names (features.md section 2.8): labels, records, and the sketch results
    /// of this run.
    pub fn with_names<T>(&self, use_names: impl FnOnce(&dyn Names) -> T) -> T {
        let outputs: BTreeMap<Id, Arc<FeatureOutput>> = self
            .report
            .features
            .iter()
            .filter_map(|report| {
                report
                    .output
                    .as_ref()
                    .map(|output| (report.feature.clone(), Arc::clone(output)))
            })
            .collect();
        let names = DocumentNames {
            document: &self.document,
            registry: self.recompute.registry().as_ref(),
            outputs: &outputs,
        };
        use_names(&names)
    }

    /// The error of `feature` rendered with `diagnosis`.
    pub fn render_error(&self, feature: &str, diagnosis: &Diagnosis) -> String {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.render(context))
    }

    /// The fixes of `feature`'s error with `diagnosis`.
    pub fn error_fixes(&self, feature: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
        let error = self.error(feature);
        self.with_context(feature, diagnosis, |context| error.fixes(context))
    }

    /// The only warning or notice of `feature` with this code, rendered with `diagnosis`.
    pub fn render_warning(&self, feature: &str, code: &str, diagnosis: &Diagnosis) -> String {
        let warning = self.warning(feature, code);
        self.with_context(feature, diagnosis, |context| warning.render(context))
    }

    /// The fixes of the only warning or notice of `feature` with this code, with `diagnosis`.
    pub fn warning_fixes(&self, feature: &str, code: &str, diagnosis: &Diagnosis) -> Vec<Fix> {
        let warning = self.warning(feature, code);
        self.with_context(feature, diagnosis, |context| warning.fixes(context))
    }

    /// The error of `feature`; fails the test when it has none.
    pub fn error(&self, feature: &str) -> &FeatureError {
        self.feature(feature)
            .error
            .as_ref()
            .unwrap_or_else(|| panic!("{feature} has no error; {}", self.summary()))
    }

    /// The only warning or notice of `feature` with this code.
    pub fn warning(&self, feature: &str, code: &str) -> &FeatureWarning {
        let matching: Vec<&FeatureWarning> = self
            .feature(feature)
            .warnings
            .iter()
            .filter(|warning| warning.code() == code)
            .collect();
        match matching.as_slice() {
            [warning] => warning,
            _ => panic!(
                "{feature} should have exactly one {code}, but its messages are {:?}",
                self.messages(feature)
            ),
        }
    }

    /// The only `plane_fallback` warning of `sketch` (features/sketch.md section 3.5): where it placed the sketch, its
    /// cause and its loss.
    pub fn plane_fallback(&self, sketch: &str) -> (&Placed, FallbackCause, Option<&Loss>) {
        let warning = self.warning(sketch, "plane_fallback");
        let FeatureWarning::PlaneFallback {
            placed,
            cause,
            loss,
            ..
        } = warning
        else {
            panic!("expected a plane fallback warning, got {warning:?}");
        };
        (placed, *cause, loss.as_ref())
    }

    /// Asserts that `feature` built with status ok and exactly these notices, in order, each of severity info
    /// (recompute.md section 3.5 step 11: notices leave the status ok).
    pub fn assert_notices(&self, feature: &str, codes: &[&str]) {
        let report = self.feature(feature);
        assert!(
            matches!(report.status, Status::Ok),
            "{feature} should be ok with notices {codes:?}, but it is {:?} with messages {:?}",
            report.status,
            self.messages(feature),
        );
        let actual: Vec<&str> = report
            .warnings
            .iter()
            .map(|warning| warning.code())
            .collect();
        assert_eq!(actual, codes, "notice codes of {feature}");
        for warning in &report.warnings {
            assert_eq!(
                warning.severity(),
                Severity::Info,
                "{feature}'s {} is a notice",
                warning.code()
            );
        }
    }

    /// Asserts that the only notice of `feature` is a `reference_followed` that followed by `follow`, at the field
    /// `pointer` (features.md section 3.8).
    pub fn assert_followed(&self, feature: &str, follow: Follow, pointer: &str) {
        self.assert_notices(feature, &["reference_followed"]);
        let warning = self.warning(feature, "reference_followed");
        assert!(
            matches!(warning, FeatureWarning::ReferenceFollowed { follow: actual, .. } if *actual == follow),
            "{feature} should follow by {follow:?}, but its notice is {warning:?}"
        );
        assert_eq!(
            message(self, feature, "reference_followed")
                .pointer
                .as_deref(),
            Some(pointer),
        );
    }

    /// `Reference::from_element` of the element named `name` in `body` just before `feature`: what a pick there
    /// stores, and what a candidate or fix of that feature holds.
    pub fn picked_before(&self, feature: &str, body: &str, name: &str) -> Reference {
        picked_in(body_before(self, feature, body), name)
    }
}

/// `Reference::from_element` of the element named `name` in `state`.
pub fn picked_in(state: &BodyState, name: &str) -> Reference {
    let set = state
        .element_set()
        .expect("the element set of a built body");
    let name = ElementName::parse(name)
        .unwrap_or_else(|error| panic!("{name} is not a valid element name: {error}"));
    let info = set
        .elements()
        .iter()
        .find(|info| info.name == name)
        .unwrap_or_else(|| panic!("the body has no element {name}"));
    Reference::from_element(info, &set)
}

/// Asserts that the edge or face axis `feature` stores at `pointer` (a JSON Pointer into its params, such as
/// `/axis`) is written as a pick of the element `name` of `body` just before `feature` stores it (features.md
/// section 6.1): its reference is `Reference::from_element` of that element, and its `direction` is the
/// direction of the element's fingerprint, which is the pick's `axis_direction` while the body has not moved
/// (engine.md section 3.9).
pub fn assert_axis_stored_as_picked(
    run: &Run,
    feature: &str,
    pointer: &str,
    body: &str,
    name: &str,
) {
    let params = Value::Object(run.record(feature).params.clone());
    let axis = params
        .pointer(pointer)
        .unwrap_or_else(|| panic!("{feature} stores no axis at {pointer}"));
    let kind = axis["kind"]
        .as_str()
        .unwrap_or_else(|| panic!("the axis at {pointer} has a kind"));
    let stored: Reference = serde_json::from_value(axis[kind].clone()).unwrap_or_else(|error| {
        panic!("the {kind} axis at {pointer} stores no reference: {error}")
    });
    let picked = run.picked_before(feature, body, name);
    assert_eq!(
        stored, picked,
        "the reference at {pointer}/{kind} as a pick stores it"
    );
    let direction: [f64; 3] = serde_json::from_value(axis["direction"].clone())
        .unwrap_or_else(|error| panic!("the axis at {pointer} stores no direction: {error}"));
    let fingerprint_direction = picked
        .fingerprint
        .direction
        .unwrap_or_else(|| panic!("the fingerprint of {name} has a direction"));
    assert_coordinates(
        &format!("the direction at {pointer}"),
        direction,
        fingerprint_direction,
        AXIS_TOLERANCE,
    );
}

/// `UseReferences` of one reference at `pointer`.
pub fn use_reference(pointer: &str, reference: Reference) -> Fix {
    Fix::UseReferences {
        pointer: pointer.to_string(),
        references: vec![reference],
    }
}

/// `Pick` of the reference at `pointer`.
pub fn pick(pointer: &str) -> Fix {
    Fix::Pick {
        pointer: pointer.to_owned(),
    }
}

/// `SetField` of this feature's field `path`, labeled `label`, to `value`.
pub fn set_field(path: &str, label: &str, value: Value) -> Fix {
    Fix::SetField {
        feature: None,
        field: FieldRef {
            path: path.to_owned(),
            label: label.to_owned(),
        },
        value,
    }
}

/// `SetField` of `reverse` to `on`: "Turn on Reverse" for true, "Turn off Reverse" for false
/// (engine.md section 3.22).
pub fn set_reverse(on: bool) -> Fix {
    set_field("reverse", "Reverse", Value::Bool(on))
}

/// `KeepMatch` of the reference at `pointer`.
pub fn keep_match(pointer: &str) -> Fix {
    Fix::KeepMatch {
        pointer: pointer.to_owned(),
    }
}

/// `RemoveEntry` of the list entry at `pointer`.
pub fn remove_entry(pointer: &str) -> Fix {
    Fix::RemoveEntry {
        pointer: pointer.to_owned(),
    }
}

/// `KeepMatch` then `Pick` at `pointer`: the fixes of a followed reference (features.md section 4.2).
pub fn keep_and_pick(pointer: &str) -> [Fix; 2] {
    [keep_match(pointer), pick(pointer)]
}

/// The successions a body's naming map records, as (stored face, faces now) names.
pub fn successions(state: &BodyState) -> Vec<(String, Vec<String>)> {
    names_now(&state.naming().successions().0)
}

/// The merge aliases a body's naming map records (naming.md section 3.3 step 2), as (former name,
/// names now) in key order.
pub fn aliases(state: &BodyState) -> Vec<(String, Vec<String>)> {
    names_now(&state.naming().aliases().0)
}

/// The entries of a naming map's successions or aliases as text, as (stored name, names now) in key
/// order.
fn names_now(entries: &BTreeMap<ElementName, Vec<ElementName>>) -> Vec<(String, Vec<String>)> {
    entries
        .iter()
        .map(|(stored, now)| {
            (
                stored.to_string(),
                now.iter().map(ToString::to_string).collect(),
            )
        })
        .collect()
}

/// The faces `feature` records absent (features.md section 2.7), as (name, why) with `why` in its `Debug` form.
pub fn absent_faces(run: &Run, feature: &str) -> Vec<(String, String)> {
    run.output(feature)
        .absent
        .iter()
        .map(|face| (face.name.to_string(), format!("{:?}", face.why)))
        .collect()
}

/// Asserts that every rendered message of `feature` passes the message lint.
pub fn assert_messages_pass_lint(run: &Run, feature: &str) {
    for text in run.messages(feature) {
        let failures = lint_failures(text);
        assert!(
            failures.is_empty(),
            "the message {text:?} of {feature} breaks the message lint: {failures:?}"
        );
    }
}

/// Asserts that `feature` failed with `code` (`not_coplanar` or `not_collinear`) about the reference of the param
/// `field`, rendered `text`, and that its fixes are `UseReferences` with each element `names` gives, as a pick in
/// `body` just before the feature stores it, then `Pick`, all at the JSON Pointer of the reference inside the param
/// (`/base/face` for `base`, `/direction/axis/edge` for `direction.axis`). Feature specs name the param ("field
/// `base`"); the error may also name the reference inside it, as `NotPlanar` does (features.md section 4.1).
#[track_caller]
pub fn assert_choice_between(
    run: &Run,
    feature: &str,
    code: &str,
    (field, reference_key): (&str, &str),
    body: &str,
    names: &[&str],
    text: &str,
) {
    let error = run.assert_error(feature, code);
    let path = &error
        .field()
        .unwrap_or_else(|| panic!("the error names no field: {error}"))
        .path;
    let reference_path = format!("{field}.{reference_key}");
    assert!(
        *path == field || *path == reference_path,
        "the error is about {path}, expected {field} or {reference_path}"
    );
    assert_eq!(run.messages(feature), [text]);
    let pointer = format!("/{}", reference_path.replace('.', "/"));
    let mut expected: Vec<Fix> = names
        .iter()
        .map(|name| use_reference(&pointer, run.picked_before(feature, body, name)))
        .collect();
    expected.push(pick(&pointer));
    assert_eq!(message(run, feature, code).fixes, expected);
    assert_messages_pass_lint(run, feature);
}
