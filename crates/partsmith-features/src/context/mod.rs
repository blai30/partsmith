//! The read-only build context and what it is made from (features.md sections 2.6, 3.4 and 3.6 to 3.8).

mod body_state;
#[cfg(test)]
mod fixtures;
mod geometry;
mod placements;
mod references;
mod regions;
mod values;

pub use body_state::BodyState;
pub use placements::{FaceFallback, FacePlacement};
pub use references::Usable;
pub use values::Values;
pub(crate) use values::expected_dimension;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use partsmith_model::{Expr, FeatureManifest, FieldUnit, Id, Units};
use partsmith_naming::{ElementInfo, ElementKind, Reference};
use partsmith_occt_sys::SubShape;
use partsmith_sketch::Succession;

use crate::error::{Bound, FeatureError, FeatureWarning, FieldRef, Loss, Noun};
use crate::output::{DatumResult, FeatureOutput, OutputData, SketchResult};
use references::Outcome;

/// Largest distance from a whole number that a count may have (section 3.4).
const WHOLE_NUMBER_TOLERANCE: f64 = 1e-9;

/// Everything a `BuildContext` is made from. Recompute fills it with only what the feature declared.
#[derive(Clone, Copy, Debug)]
pub struct BuildInputs<'a> {
    /// The feature being built.
    pub feature: &'a Id,
    /// Its manifest, used for field labels in errors.
    pub manifest: &'a FeatureManifest,
    /// Document units.
    pub units: Units,
    /// Pre-evaluated values of the declared expressions.
    pub values: &'a Values,
    /// State of every declared body just before this feature.
    pub bodies: &'a BTreeMap<Id, BodyState>,
    /// For each declared body, its writer chain: for each earlier active writer that ran, was reused or failed, in
    /// timeline order, the writer's id and the body's state just before it (recompute.md section 3.5).
    pub history: &'a BTreeMap<Id, Vec<(Id, BodyState)>>,
    /// Output of every declared feature dependency that built successfully.
    pub upstream: &'a BTreeMap<Id, Arc<FeatureOutput>>,
    /// Bytes of every declared asset (recompute checks they exist before building).
    pub assets: &'a BTreeMap<String, Arc<[u8]>>,
    /// Cancellation flag of the current run.
    pub cancel: &'a CancelToken,
}

/// Read-only inputs of one feature build, assembled by recompute.
///
/// The context only exposes what the feature declared: evaluating an undeclared expression, or
/// reading an undeclared body, feature or asset, is `UndeclaredDependency` (section 3.1).
#[derive(Clone, Copy, Debug)]
pub struct BuildContext<'a> {
    /// What the context is made from.
    inputs: BuildInputs<'a>,
}

impl<'a> BuildContext<'a> {
    /// Creates a context. Used by recompute and by tests.
    pub fn new(inputs: BuildInputs<'a>) -> Self {
        BuildContext { inputs }
    }

    /// Id of the feature being built.
    pub fn feature(&self) -> &'a Id {
        self.inputs.feature
    }

    /// Document units.
    pub fn units(&self) -> Units {
        self.inputs.units
    }

    /// The field at `path` with its label from the feature's manifest, for errors and warnings.
    pub fn field(&self, path: &str) -> FieldRef {
        FieldRef::new(path, self.inputs.manifest)
    }

    /// The declared length at `field` in millimeters.
    pub fn eval_length(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError> {
        self.evaluated(field, expr, FieldUnit::Length)
    }

    /// The declared angle at `field` in radians.
    pub fn eval_angle(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError> {
        self.evaluated(field, expr, FieldUnit::Angle)
    }

    /// The declared plain number at `field`.
    pub fn eval_scalar(&self, field: &str, expr: &Expr) -> Result<f64, FeatureError> {
        self.evaluated(field, expr, FieldUnit::Scalar)
    }

    /// The declared whole number at `field`, at least `minimum` (section 3.4). The range is
    /// checked before the whole number, so every fix a message proposes is a count that works.
    pub fn eval_count(&self, field: &str, expr: &Expr, minimum: u32) -> Result<u32, FeatureError> {
        let value = self.evaluated(field, expr, FieldUnit::Count)?;
        count(value, minimum).map_err(|problem| problem.error(self.field(field), value))
    }

    /// State of a declared body just before this feature.
    pub fn body(&self, body: &Id) -> Result<&'a BodyState, FeatureError> {
        self.inputs
            .bodies
            .get(body)
            .ok_or_else(|| undeclared_body(body))
    }

    /// The writer chain of a declared body (`BuildInputs::history`), oldest first: each earlier writer with the
    /// body's state just before it. An undeclared body gives `UndeclaredDependency`.
    pub fn body_history(&self, body: &Id) -> Result<&'a [(Id, BodyState)], FeatureError> {
        if !self.inputs.bodies.contains_key(body) {
            return Err(undeclared_body(body));
        }
        Ok(self.inputs.history.get(body).map_or(&[][..], Vec::as_slice))
    }

    /// Result of a declared sketch dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn sketch(&self, field: &str, feature: &Id) -> Result<&'a SketchResult, FeatureError> {
        match &self.upstream(feature)?.data {
            OutputData::Sketch(sketch) => Ok(sketch),
            _ => Err(self.wrong_feature_type(field, feature, Noun::Sketch)),
        }
    }

    /// Result of a declared datum dependency named at `field`. Another kind of output gives `WrongFeatureType`.
    pub fn datum(&self, field: &str, feature: &Id) -> Result<&'a DatumResult, FeatureError> {
        match &self.upstream(feature)?.data {
            OutputData::Datum(datum) => Ok(datum),
            _ => Err(self.wrong_feature_type(field, feature, Noun::DatumPlane)),
        }
    }

    /// Bytes of a declared asset named at `field`. An undeclared name gives `UndeclaredDependency`.
    pub fn asset(&self, field: &str, name: &str) -> Result<&'a [u8], FeatureError> {
        self.inputs
            .assets
            .get(name)
            .map(|bytes| &**bytes)
            .ok_or_else(|| FeatureError::UndeclaredDependency {
                what: format!("the asset '{name}' at '{field}'"),
            })
    }

    /// Resolves one required reference to its elements, in resolver order (section 3.8). Missing or ambiguous is
    /// `ReferenceNotFound` with its loss; a guess, a widening or a follow adds its warning or notice to the result;
    /// an empty body is `BodyEmpty`.
    pub fn resolve(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
    ) -> Result<Resolved, FeatureError> {
        self.resolve_usable(field, body, reference, kind, &every_element)
    }

    /// Resolves a list of references (section 3.8). Missing or ambiguous entries become `ReferenceLost` warnings and
    /// are skipped; guesses, widenings and follows add their warnings and notices; a non-empty list that resolves to
    /// nothing is `ReferencesNotFound` with every loss, in reference order.
    pub fn resolve_all(
        &self,
        field: &str,
        body: &Id,
        references: &[Reference],
        kind: ElementKind,
    ) -> Result<Resolved, FeatureError> {
        self.resolve_all_usable(field, body, references, kind, &every_element)
    }

    /// `resolve_all` for a field that cannot use every element: candidates that `usable` rejects are dropped from
    /// every loss and guess, so a fix is offered only when it works (section 3.8; fillet and chamfer drop smooth
    /// edges).
    pub fn resolve_all_usable(
        &self,
        field: &str,
        body: &Id,
        references: &[Reference],
        kind: ElementKind,
        usable: &Usable<'_>,
    ) -> Result<Resolved, FeatureError> {
        let resolver = self.entry_resolver(field, body, references, kind, usable)?;
        let outcomes = (0..references.len())
            .map(|position| resolver.entry(position, self.field(&format!("{field}[{position}]"))))
            .collect::<Result<Vec<Outcome>, FeatureError>>()?;
        if !references.is_empty() && outcomes.iter().all(Outcome::has_no_elements) {
            return Err(FeatureError::ReferencesNotFound {
                field: self.field(field),
                kind,
                losses: outcomes
                    .into_iter()
                    .filter_map(Outcome::into_loss)
                    .collect(),
            });
        }
        Ok(Resolved::of_entries(outcomes))
    }

    /// `resolve` for a field that cannot use every element (section 3.8).
    fn resolve_usable(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
        usable: &Usable<'_>,
    ) -> Result<Resolved, FeatureError> {
        self.resolve_one(field, body, reference, kind, usable)?
            .map_err(|loss| FeatureError::ReferenceNotFound {
                field: self.field(field),
                loss,
            })
    }

    /// Resolves one reference, whose messages name `field` itself (section 3.8): its elements with the warning or
    /// notice of a guess, a widening or a follow, or its loss when it is missing or ambiguous. An empty body is
    /// `BodyEmpty`.
    fn resolve_one(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
        usable: &Usable<'_>,
    ) -> Result<Result<Resolved, Loss>, FeatureError> {
        let references = std::slice::from_ref(reference);
        let resolver = self.entry_resolver(field, body, references, kind, usable)?;
        Ok(match resolver.entry(0, self.field(field))? {
            Outcome::Resolved(resolved) => Ok(resolved),
            Outcome::Lost { loss, .. } => Err(*loss),
        })
    }

    /// True once the run has been canceled. Long builds check it between steps.
    pub fn is_canceled(&self) -> bool {
        self.inputs.cancel.is_canceled()
    }

    /// `Canceled` once the run has been canceled, for the framework's own long steps.
    pub(crate) fn check_canceled(&self) -> Result<(), FeatureError> {
        if self.is_canceled() {
            Err(FeatureError::Canceled)
        } else {
            Ok(())
        }
    }

    /// The value of a declared use, if `field` was declared with exactly this expression and unit.
    fn evaluated(&self, field: &str, expr: &Expr, kind: FieldUnit) -> Result<f64, FeatureError> {
        match self.inputs.values.get(field) {
            Some((declared, value)) if declared.expr == *expr && declared.kind == kind => {
                value.clone()
            }
            _ => Err(FeatureError::UndeclaredDependency {
                what: format!("the {} '{expr}' at '{field}'", unit_noun(kind)),
            }),
        }
    }

    /// The output of a declared feature dependency.
    fn upstream(&self, feature: &Id) -> Result<&'a FeatureOutput, FeatureError> {
        self.inputs
            .upstream
            .get(feature)
            .map(Arc::as_ref)
            .ok_or_else(|| FeatureError::UndeclaredDependency {
                what: format!("feature '{feature}'"),
            })
    }

    /// The error for a feature whose output is not what `field` needs.
    fn wrong_feature_type(&self, field: &str, feature: &Id, expected: Noun) -> FeatureError {
        FeatureError::WrongFeatureType {
            field: self.field(field),
            feature: feature.clone(),
            expected,
        }
    }
}

/// Result of resolving one reference or a list of references (section 3.8).
#[derive(Clone, Debug)]
pub struct Resolved {
    /// Resolved sub-shapes (kernel indices) in reference order, then resolver order, without duplicates.
    pub elements: Vec<SubShape>,
    /// For each element, the position in the reference list of the reference it was first resolved from.
    pub sources: Vec<usize>,
    /// Warnings of severity `Warning`: one per skipped reference (`ReferenceLost`), guess (`ReferenceGuessed`) or
    /// widening (`ReferenceWidened`), in reference order.
    pub warnings: Vec<FeatureWarning>,
    /// Notices of severity `Info` for references that followed a change (`ReferenceFollowed`), in reference order.
    pub notices: Vec<FeatureWarning>,
}

impl Resolved {
    /// The warnings, then the notices: the messages a feature adds to its output (section 3.8).
    pub fn into_messages(self) -> Vec<FeatureWarning> {
        let mut messages = self.warnings;
        messages.extend(self.notices);
        messages
    }
}

/// Result of resolving a profile (section 3.7).
#[derive(Clone, Debug)]
pub struct ResolvedProfile<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// Indices into the sketch result's regions, in profile order, without duplicates.
    pub regions: Vec<usize>,
    /// Successions of the regions matched by boundary (sketch.md section 3.14), sorted by entity, without
    /// duplicates.
    pub successions: Vec<Succession>,
    /// Warnings for resized, remapped, ambiguous or missing regions.
    pub warnings: Vec<FeatureWarning>,
    /// Notices for regions split by other curves of the sketch (`RegionSplit`).
    pub notices: Vec<FeatureWarning>,
}

impl ResolvedProfile<'_> {
    /// The warnings, then the notices: the messages a sweep reports first (section 3.7).
    pub fn into_messages(self) -> Vec<FeatureWarning> {
        let mut messages = self.warnings;
        messages.extend(self.notices);
        messages
    }
}

/// A value resolved through stored references, with the warnings and notices they gave (section 3.8). The caller
/// adds `warnings` to its output.
#[derive(Clone, Debug, PartialEq)]
pub struct WithWarnings<T> {
    /// The resolved value.
    pub value: T,
    /// Warnings and info notices, in the order raised.
    pub warnings: Vec<FeatureWarning>,
}

impl<T> WithWarnings<T> {
    /// A value resolved without references.
    fn alone(value: T) -> WithWarnings<T> {
        WithWarnings {
            value,
            warnings: Vec::new(),
        }
    }
}

/// Shared cancellation flag for one run. Cloning shares the flag.
#[derive(Clone, Debug, Default)]
pub struct CancelToken {
    /// The flag shared by every clone.
    canceled: Arc<AtomicBool>,
}

impl CancelToken {
    /// A token that is not canceled.
    pub fn new() -> Self {
        CancelToken::default()
    }

    /// Requests cancellation. Idempotent.
    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::Relaxed);
    }

    /// True once `cancel` was called on this token or a clone.
    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Relaxed)
    }
}

/// `UndeclaredDependency` for a body the feature did not declare.
fn undeclared_body(body: &Id) -> FeatureError {
    FeatureError::UndeclaredDependency {
        what: format!("body '{body}'"),
    }
}

/// Every element is usable: the fields of `resolve` and `resolve_all`.
fn every_element(_info: &ElementInfo) -> Result<bool, FeatureError> {
    Ok(true)
}

/// Why a value is not a valid count.
enum CountProblem {
    /// It is outside the u32 range.
    OutOfRange(Bound),
    /// It is not within 1e-9 of a whole number.
    NotWhole,
}

impl CountProblem {
    /// The feature error for `value` at `field`.
    fn error(self, field: FieldRef, value: f64) -> FeatureError {
        match self {
            CountProblem::OutOfRange(bound) => FeatureError::OutOfRange {
                field,
                kind: FieldUnit::Count,
                value,
                bound,
                bound_name: None,
            },
            CountProblem::NotWhole => FeatureError::NotWholeNumber { field, value },
        }
    }
}

/// The count `value` stands for (section 3.4): the nearest whole number when `value` is within
/// 1e-9 of it, checked in order against `minimum`, the u32 range and being whole. The range comes
/// first, so a fraction below the minimum or above the range reports the limit it misses.
fn count(value: f64, minimum: u32) -> Result<u32, CountProblem> {
    let nearest = value.round();
    let whole = (value - nearest).abs() <= WHOLE_NUMBER_TOLERANCE;
    let checked = if whole { nearest } else { value };
    if checked < f64::from(minimum) {
        return Err(CountProblem::OutOfRange(Bound::AtLeast(f64::from(minimum))));
    }
    if checked > f64::from(u32::MAX) {
        return Err(CountProblem::OutOfRange(Bound::AtMost(f64::from(u32::MAX))));
    }
    if !whole {
        return Err(CountProblem::NotWhole);
    }
    // `nearest` is a whole number in 0..=u32::MAX here, so the conversion is exact.
    Ok(nearest as u32)
}

/// The noun of a value kind in an `UndeclaredDependency` description.
fn unit_noun(kind: FieldUnit) -> &'static str {
    match kind {
        FieldUnit::Length => "length",
        FieldUnit::Angle => "angle",
        FieldUnit::Scalar => "number",
        FieldUnit::Count => "count",
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::Point;
    use partsmith_sketch::OriginPlane;
    use serde_json::json;

    use super::fixtures::{
        ANY_BOX_FACE, GONE_IMPORTED_FACE, LOST_FACE, box_fixture, imported_box_fixture,
        rectangle_with_circle, reference, sketch_fixture, sketch_output,
    };
    use super::*;
    use crate::params::ProfileRef;
    use crate::test_support::{Fixture, id, manifest, values};

    fn profile(regions: serde_json::Value) -> ProfileRef {
        serde_json::from_value(json!({"sketch": "f-sk1", "regions": regions}))
            .expect("the profile deserializes")
    }

    fn codes(warnings: &[FeatureWarning]) -> Vec<&'static str> {
        warnings.iter().map(FeatureWarning::code).collect()
    }

    #[test]
    fn an_ambiguous_required_reference_lists_its_candidates() {
        let fixture = imported_box_fixture();

        let error = fixture
            .context()
            .resolve(
                "plane.face",
                &id("b-1"),
                &reference(GONE_IMPORTED_FACE, ANY_BOX_FACE),
                ElementKind::Face,
            )
            .expect_err("six faces match equally");

        // features.md section 4.1: an ambiguous reference is a loss whose candidates are the faces it matches.
        let FeatureError::ReferenceNotFound { loss, .. } = &error else {
            panic!("expected ReferenceNotFound, got {error:?}");
        };
        assert!(loss.ambiguous);
        assert_eq!(loss.candidates.len(), 6);
        assert!(matches!(
            error.repair(),
            Some(crate::error::Repair::Element { loss, .. }) if loss.ambiguous
        ));
    }

    #[test]
    fn an_ambiguous_listed_reference_is_skipped_with_its_candidates() {
        let fixture = imported_box_fixture();
        let faces = [
            reference(GONE_IMPORTED_FACE, ANY_BOX_FACE),
            reference("f-imp1/face#4", "plane|n=0,0,1"),
        ];

        let resolved = fixture
            .context()
            .resolve_all("remove_faces", &id("b-1"), &faces, ElementKind::Face)
            .expect("the top resolves");

        // features.md section 3.8: an ambiguous entry is a lost one, whose candidates leave out the face the
        // second entry resolves to.
        assert_eq!(resolved.elements.len(), 1);
        assert_eq!(codes(&resolved.warnings), ["reference_ambiguous"]);
        let FeatureWarning::ReferenceLost { field, loss } = &resolved.warnings[0] else {
            panic!("expected ReferenceLost, got {:?}", resolved.warnings[0]);
        };
        assert_eq!(loss.position, 0);
        assert!(loss.ambiguous);
        assert_eq!(loss.candidates.len(), 5);
        // Section 4.2: the message counts all six faces that fit, the one the second entry uses included.
        assert_eq!(loss.matches, 6);
        assert!(
            resolved.warnings[0]
                .to_string()
                .contains(" now matches 6 faces, so it was skipped."),
            "{}",
            resolved.warnings[0]
        );
        assert_eq!(field.pointer(), "/remove_faces/0");
    }

    #[test]
    fn an_ambiguous_profile_region_is_skipped() {
        let mut output = sketch_output(OriginPlane::Xy.placement(), rectangle_with_circle());
        let OutputData::Sketch(sketch) = &mut Arc::make_mut(&mut output).data else {
            unreachable!("sketch_output makes a sketch");
        };
        let disc = sketch
            .profile
            .regions
            .iter()
            .find(|region| region.key.to_string() == "e-5")
            .expect("the circle bounds a region")
            .clone();
        sketch.profile.regions.push(disc);
        let mut fixture = Fixture::new("f-ext1");
        fixture.upstream.insert(id("f-sk1"), output);
        let context = fixture.context();

        let resolved = context
            .profile(
                "profile",
                &profile(json!([
                    {"key": "e-5", "fp": "c=40,25|a=314.159265"},
                    {"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}
                ])),
            )
            .expect("the second region resolves");

        assert_eq!(resolved.regions.len(), 1);
        assert_eq!(codes(&resolved.warnings), ["region_ambiguous"]);
        assert!(matches!(
            resolved.warnings[0].repair(),
            Some(crate::error::Repair::Region { pointer, ambiguous: true, .. }) if pointer == "/profile/regions/0"
        ));
    }

    #[test]
    fn resolve_finds_a_face_by_name() {
        let fixture = box_fixture();

        let found = fixture
            .context()
            .resolve(
                "face",
                &id("b-1"),
                &reference("f-box1/top", "plane|n=0,0,1"),
                ElementKind::Face,
            )
            .expect("the top face resolves");

        assert_eq!(found.elements.len(), 1);
        let shape = fixture.bodies[&id("b-1")].shape().expect("a solid");
        let geometry = shape
            .element(found.elements[0].kind, found.elements[0].index)
            .expect("geometry");
        assert_eq!(geometry.centroid, Point::new(5.0, 5.0, 10.0));
    }

    #[test]
    fn resolve_of_a_lost_face_has_repair_data() {
        let fixture = box_fixture();

        let error = fixture
            .context()
            .resolve(
                "plane.face",
                &id("b-1"),
                &reference("f-box1/nothing", LOST_FACE),
                ElementKind::Face,
            )
            .expect_err("the face is lost");

        assert_eq!(error.code(), "reference_not_found");
        assert!(matches!(
            error.repair(),
            Some(crate::error::Repair::Element { pointer, loss, .. }) if pointer == "/plane/face" && !loss.ambiguous
        ));
    }

    #[test]
    fn resolve_in_an_empty_body_is_body_empty() {
        let fixture = Fixture::new("f-test1").with_body("b-1", BodyState::empty());
        let context = fixture.context();

        let error = context
            .resolve(
                "face",
                &id("b-1"),
                &reference("f-box1/top", "plane|n=0,0,1"),
                ElementKind::Face,
            )
            .expect_err("the body is empty");

        assert_eq!(
            error,
            FeatureError::BodyEmpty {
                field: context.field("body"),
                body: id("b-1"),
                use_new: false
            }
        );
    }

    #[test]
    fn resolve_all_skips_lost_references_with_a_warning() {
        let fixture = box_fixture();
        let edges = [
            reference("edge(f-box1/front|f-box1/top)", "line|n=1,0,0"),
            reference(
                "edge(f-box1/front|f-box1/nothing)",
                "line|n=0,0,1|c=500,500,500|l=1",
            ),
            reference("edge(f-box1/front|f-box1/top)", "line|n=1,0,0"),
        ];

        let resolved = fixture
            .context()
            .resolve_all("edges", &id("b-1"), &edges, ElementKind::Edge)
            .expect("one edge resolves");

        assert_eq!(resolved.elements.len(), 1);
        assert_eq!(codes(&resolved.warnings), ["reference_missing"]);
        assert_eq!(
            resolved.warnings[0]
                .field()
                .map(FieldRef::pointer)
                .as_deref(),
            Some("/edges/1")
        );
    }

    #[test]
    fn resolve_all_with_nothing_found_counts_the_references() {
        let fixture = box_fixture();
        let edges = [
            reference("edge(f-box1/a|f-box1/b)", "line|n=0,0,1|c=500,500,500|l=1"),
            reference("edge(f-box1/c|f-box1/d)", "line|n=0,0,1|c=500,500,500|l=1"),
        ];

        let error = fixture
            .context()
            .resolve_all("edges", &id("b-1"), &edges, ElementKind::Edge)
            .expect_err("nothing resolves");

        assert!(matches!(
            error,
            FeatureError::ReferencesNotFound { kind: ElementKind::Edge, ref field, ref losses } if field.path == "edges" && losses.len() == 2
        ));
    }

    #[test]
    fn a_reference_of_another_kind_is_missing() {
        let fixture = box_fixture();
        let faces = [reference("f-box1/top", "plane|n=0,0,1")];

        let error = fixture
            .context()
            .resolve_all("edges", &id("b-1"), &faces, ElementKind::Edge)
            .expect_err("a face is not an edge");

        assert_eq!(error.code(), "references_not_found");
    }

    #[test]
    fn profile_regions_resolve_by_key() {
        let fixture = sketch_fixture();
        let context = fixture.context();

        let resolved = context
            .profile(
                "profile",
                &profile(json!([{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}])),
            )
            .expect("the region resolves");

        assert_eq!(resolved.regions.len(), 1);
        assert!(resolved.warnings.is_empty());
    }

    #[test]
    fn profile_regions_found_otherwise_are_remapped_and_used_once() {
        let fixture = sketch_fixture();
        let context = fixture.context();

        let resolved = context
            .profile(
                "profile",
                &profile(json!([
                    {"key": "e-5", "fp": "c=40,25|a=314.159265"},
                    {"key": "e-10,e-6,e-7,e-8;~e-5", "fp": "c=40,25|a=3685.840735"},
                    {"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}
                ])),
            )
            .expect("the regions resolve");

        assert_eq!(resolved.regions.len(), 2);
        assert_eq!(codes(&resolved.warnings), ["region_remapped"]);
        assert_eq!(
            resolved.warnings[0]
                .field()
                .map(FieldRef::pointer)
                .as_deref(),
            Some("/profile/regions/1")
        );
    }

    #[test]
    fn missing_profile_regions_are_skipped_or_fail_together() {
        let fixture = sketch_fixture();
        let context = fixture.context();
        let lost = json!({"key": "e-7,e-8", "fp": "c=200,200|a=100"});

        let partly = context
            .profile(
                "profile",
                &profile(json!([lost, {"key": "e-5", "fp": "c=40,25|a=314.159265"}])),
            )
            .expect("one region resolves");
        let error = context
            .profile("profile", &profile(json!([lost])))
            .expect_err("nothing resolves");

        assert_eq!(codes(&partly.warnings), ["region_missing"]);
        // features.md section 3.7: the error keeps every region it lost.
        let FeatureError::ProfileNotFound {
            field,
            sketch,
            regions,
        } = &error
        else {
            panic!("expected ProfileNotFound, got {error:?}");
        };
        assert_eq!(field, &FieldRef::new("profile", &manifest(Vec::new())));
        assert_eq!(sketch, &id("f-sk1"));
        let positions: Vec<usize> = regions.iter().map(|region| region.position).collect();
        assert_eq!(positions, [0]);
        assert!(regions[0].loss.is_some());
    }

    #[test]
    fn profile_faces_of_a_region_with_a_hole() {
        let fixture = sketch_fixture();
        let context = fixture.context();
        let resolved = context
            .profile(
                "profile",
                &profile(json!([{"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"}])),
            )
            .expect("the region resolves");

        let faces = resolved.faces().expect("the profile builds");

        let tags: Vec<&str> = faces
            .profile
            .edge_tags
            .iter()
            .map(|&tag| faces.entities[tag as usize].as_str())
            .collect();
        assert_eq!(
            faces
                .profile
                .shape
                .count(partsmith_occt_sys::SubShapeKind::Face),
            1
        );
        assert_eq!(tags.len(), 5);
        assert!(tags.contains(&"e-5"));
        let area = faces.profile.shape.area().expect("the face has an area");
        assert!((area - (4000.0 - std::f64::consts::PI * 100.0)).abs() < 1e-6);
    }

    #[test]
    fn profile_faces_of_two_regions_are_their_union() {
        let fixture = sketch_fixture();
        let context = fixture.context();
        let resolved = context
            .profile(
                "profile",
                &profile(json!([
                    {"key": "e-1,e-2,e-3,e-4;~e-5", "fp": "c=40,25|a=3685.840735"},
                    {"key": "e-5", "fp": "c=40,25|a=314.159265"}
                ])),
            )
            .expect("the regions resolve");

        let faces = resolved.faces().expect("the profile builds");

        assert_eq!(
            faces
                .profile
                .shape
                .count(partsmith_occt_sys::SubShapeKind::Face),
            1
        );
        assert_eq!(faces.profile.edge_tags.len(), 4);
        assert!(!faces.entities.contains(&id("e-5")));
        let area = faces.profile.shape.area().expect("the face has an area");
        assert!((area - 4000.0).abs() < 1e-6);
    }

    #[test]
    fn field_labels_come_from_the_manifest() {
        let mut fixture = Fixture::new("f-test1");
        fixture.manifest = manifest(vec![partsmith_model::FieldHint {
            path: "edges".to_string(),
            label: "Edges".to_string(),
            essential: true,
            unit: None,
            selection: None,
            options: Vec::new(),
        }]);

        assert_eq!(fixture.context().field("edges[3]").label, "Edges");
        assert_eq!(fixture.context().feature(), &id("f-test1"));
        assert_eq!(fixture.context().units(), Units::Mm);
    }

    #[test]
    fn counts_within_tolerance_of_a_whole_number_are_that_number() {
        assert!(matches!(count(2.9999999999, 0), Ok(3)));
        assert!(matches!(count(-0.0000000001, 0), Ok(0)));
        assert!(matches!(count(0.9999999999, 1), Ok(1)));
        assert!(matches!(count(4294967295.0, 0), Ok(u32::MAX)));
    }

    #[test]
    fn counts_below_the_minimum_report_the_minimum() {
        assert!(matches!(
            count(0.5, 1),
            Err(CountProblem::OutOfRange(Bound::AtLeast(limit))) if limit == 1.0
        ));
        assert!(matches!(
            count(0.0, 1),
            Err(CountProblem::OutOfRange(Bound::AtLeast(limit))) if limit == 1.0
        ));
        assert!(matches!(
            count(-2.5, 0),
            Err(CountProblem::OutOfRange(Bound::AtLeast(limit))) if limit == 0.0
        ));
    }

    #[test]
    fn counts_above_the_u32_range_report_its_end() {
        assert!(matches!(
            count(4294967295.5, 0),
            Err(CountProblem::OutOfRange(Bound::AtMost(limit))) if limit == 4294967295.0
        ));
    }

    #[test]
    fn eval_count_checks_the_minimum_the_feature_gives() {
        let mut fixture = Fixture::new("f-lp1");
        fixture.values = values(&[("count", "0.5", FieldUnit::Count)]);
        let context = fixture.context();
        let expr = Expr::new("0.5");

        let error = context
            .eval_count("count", &expr, 1)
            .expect_err("0.5 is below 1");

        assert_eq!(
            error.to_string(),
            "count must be at least 1, but it is 0.5. Use 1 or more."
        );
        assert_eq!(error.suggestion(Units::Mm).as_deref(), Some("1"));
        assert_eq!(
            context
                .eval_count("count", &expr, 0)
                .map_err(|error| error.code()),
            Err("not_whole_number")
        );
    }

    #[test]
    fn eval_count_of_a_whole_number_is_the_count() {
        let mut fixture = Fixture::new("f-lp1");
        fixture.values = values(&[("count", "3", FieldUnit::Count)]);

        let count = fixture.context().eval_count("count", &Expr::new("3"), 1);

        assert_eq!(count, Ok(3));
    }

    #[test]
    fn clones_share_their_cancel_flag() {
        let token = CancelToken::new();
        let clone = token.clone();

        clone.cancel();
        clone.cancel();

        assert!(token.is_canceled());
    }
}
