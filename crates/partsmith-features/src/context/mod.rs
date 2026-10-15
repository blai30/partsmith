//! The read-only build context and what it is made from (features.md sections 2.6, 3.4 and 3.6 to 3.8).

mod body_state;
mod references;
mod regions;
mod values;

pub use body_state::BodyState;
pub use references::Usable;
pub use values::Values;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use partsmith_model::{Expr, FeatureManifest, FieldUnit, Id, LINEAR_TOLERANCE, Units};
use partsmith_naming::{
    ElementInfo, ElementKind, GeometryKind as NameGeometry, Reference, Resolution,
};
use partsmith_occt_sys::{Axis, ElementGeometry, GeometryKind, Point, SubShape, Vector};
use partsmith_sketch::{Geometry, OriginPlane, Placement, Succession};

use crate::error::{Bound, FallbackReason, FeatureError, FeatureWarning, FieldRef, Noun};
use crate::output::{DatumResult, FeatureOutput, OutputData, SketchResult};
use crate::params::{AxisRef, PlaneRef, point, vector};

/// Largest distance from a whole number that a count may have (section 3.4).
const WHOLE_NUMBER_TOLERANCE: f64 = 1e-9;

/// Components at most this large count as zero when an axis direction is canonicalized (section 3.6).
const AXIS_SIGN_TOLERANCE: f64 = 1e-9;

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
        let resolved = self.resolve_references(field, body, references, kind, true, usable)?;
        if references.is_empty() || !resolved.elements.is_empty() {
            return Ok(resolved);
        }
        Err(FeatureError::ReferencesNotFound {
            field: self.field(field),
            kind,
            losses: resolved
                .warnings
                .into_iter()
                .filter_map(|warning| match warning {
                    FeatureWarning::ReferenceLost { loss, .. } => Some(loss),
                    _ => None,
                })
                .collect(),
        })
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
        let mut resolved = self.resolve_references(
            field,
            body,
            std::slice::from_ref(reference),
            kind,
            false,
            usable,
        )?;
        let lost = resolved
            .warnings
            .iter()
            .position(|warning| matches!(warning, FeatureWarning::ReferenceLost { .. }));
        if let Some(position) = lost
            && let FeatureWarning::ReferenceLost { field, loss } =
                resolved.warnings.remove(position)
        {
            return Err(FeatureError::ReferenceNotFound { field, loss });
        }
        Ok(resolved)
    }

    /// Resolves a plane to a placement (section 3.6), with the warnings and notices of its face reference. A face
    /// uses the face placement rule.
    pub fn plane(
        &self,
        field: &str,
        plane: &PlaneRef,
    ) -> Result<WithWarnings<Placement>, FeatureError> {
        match plane {
            PlaneRef::Origin { plane } => Ok(WithWarnings::alone(plane.placement())),
            PlaneRef::Datum { feature } => {
                match self.datum(&format!("{field}.feature"), feature)? {
                    DatumResult::Plane(placement) => Ok(WithWarnings::alone(*placement)),
                }
            }
            PlaneRef::Face { body, face } => {
                let face_field = format!("{field}.face");
                let (geometry, warnings) =
                    self.first_element(&face_field, body, face, ElementKind::Face, &is_flat)?;
                let placement =
                    on_face(&geometry, &OriginPlane::Xy.placement())?.ok_or_else(|| {
                        FeatureError::NotPlanar {
                            field: self.field(&face_field),
                        }
                    })?;
                Ok(WithWarnings {
                    value: placement,
                    warnings,
                })
            }
        }
    }

    /// Places a plane on a flat face of a declared body with `hint` as the reference placement
    /// (section 3.6): the face placement rule with `hint` in place of the XY placement. The inner
    /// `Err` is the reason the face cannot place it (empty body or missing reference, ambiguous
    /// reference, face not flat), so the caller can fall back; the outer `Err` is for failures no
    /// fallback fixes, such as an undeclared body or a kernel error while reading the face.
    pub fn face_placement(
        &self,
        body: &Id,
        face: &Reference,
        hint: &Placement,
    ) -> Result<Result<Placement, FallbackReason>, FeatureError> {
        let state = self.body(body)?;
        let Some(shape) = state.shape() else {
            return Ok(Err(FallbackReason::Missing));
        };
        if face.query.kind() != ElementKind::Face {
            return Ok(Err(FallbackReason::Missing));
        }
        let set = state.element_set()?;
        let first = match partsmith_naming::resolve(face, &set) {
            Resolution::Resolved { elements, .. } => elements.into_iter().next(),
            Resolution::Ambiguous { .. } => return Ok(Err(FallbackReason::Ambiguous)),
            Resolution::Missing { .. } => return Ok(Err(FallbackReason::Missing)),
        };
        let index = first
            .and_then(|name| state.naming().index(&name))
            .ok_or_else(|| FeatureError::internal("a resolved face has no index"))?;
        let geometry = shape.element(partsmith_occt_sys::SubShapeKind::Face, index)?;
        Ok(on_face(&geometry, hint)?.ok_or(FallbackReason::NotPlanar))
    }

    /// Resolves an axis to a unit direction through a point (section 3.6), with the warnings and notices of its
    /// edge or face reference. Edge and face axes are canonicalized: the first component above 1e-9 in absolute
    /// value is positive, and the origin is the point of the line closest to the world origin.
    pub fn axis(&self, field: &str, axis: &AxisRef) -> Result<WithWarnings<Axis>, FeatureError> {
        match axis {
            AxisRef::Origin { axis } => Ok(WithWarnings::alone(Axis {
                origin: Point::new(0.0, 0.0, 0.0),
                direction: vector(axis.direction()),
            })),
            AxisRef::SketchLine { sketch, line } => {
                Ok(WithWarnings::alone(self.sketch_line(field, sketch, line)?))
            }
            AxisRef::Edge { body, edge, .. } => {
                let edge_field = format!("{field}.edge");
                let (geometry, warnings) =
                    self.first_element(&edge_field, body, edge, ElementKind::Edge, &is_straight)?;
                let value = match (geometry.kind, geometry.direction) {
                    (GeometryKind::Line, Some(direction)) => {
                        canonical_axis(geometry.centroid, direction)
                    }
                    _ => Err(FeatureError::NotStraight {
                        field: self.field(&edge_field),
                    }),
                }?;
                Ok(WithWarnings { value, warnings })
            }
            AxisRef::Face { body, face, .. } => {
                let face_field = format!("{field}.face");
                let (geometry, warnings) =
                    self.first_element(&face_field, body, face, ElementKind::Face, &has_axis)?;
                let direction = match geometry.kind {
                    GeometryKind::Cylinder | GeometryKind::Cone | GeometryKind::Torus => {
                        geometry.direction
                    }
                    GeometryKind::Sphere => Some(Vector::new(0.0, 0.0, 1.0)),
                    _ => None,
                };
                let value = match (geometry.location, direction) {
                    (Some(location), Some(direction)) => canonical_axis(location, direction),
                    _ => Err(FeatureError::NoAxis {
                        field: self.field(&face_field),
                    }),
                }?;
                Ok(WithWarnings { value, warnings })
            }
        }
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

    /// The geometry of the first element a required reference resolves to, with the reference's warnings and
    /// notices.
    fn first_element(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
        usable: &Usable<'_>,
    ) -> Result<(ElementGeometry, Vec<FeatureWarning>), FeatureError> {
        let resolved = self.resolve_usable(field, body, reference, kind, usable)?;
        let first = resolved
            .elements
            .first()
            .copied()
            .ok_or_else(|| FeatureError::internal("a resolved reference has no element"))?;
        let shape = self
            .body(body)?
            .shape()
            .ok_or_else(|| FeatureError::internal("a body with a solid has no shape"))?;
        let geometry = shape.element(first.kind, first.index)?;
        let mut warnings = resolved.warnings;
        warnings.extend(resolved.notices);
        Ok((geometry, warnings))
    }

    /// The axis of a sketch line: from its solved start to its end, in world space. An unknown
    /// entity or one that is not a line names the axis field itself (features/revolve.md test 21).
    fn sketch_line(&self, field: &str, sketch: &Id, line: &Id) -> Result<Axis, FeatureError> {
        let result = self.sketch(&format!("{field}.sketch"), sketch)?;
        let entity = result
            .solved
            .entities
            .iter()
            .find(|entity| entity.id == *line)
            .ok_or_else(|| FeatureError::UnknownEntity {
                field: self.field(field),
                sketch: sketch.clone(),
                entity: line.to_string(),
                noun: Noun::Line,
            })?;
        let not_a_line = || FeatureError::NotALine {
            field: self.field(field),
            sketch: sketch.clone(),
            entity: line.clone(),
        };
        let Geometry::Line { start, end } = entity.geometry else {
            return Err(not_a_line());
        };
        let start = result.to_world(start);
        let end = result.to_world(end);
        let direction = Vector::new(end[0] - start[0], end[1] - start[1], end[2] - start[2]);
        let length = direction.length();
        if length < LINEAR_TOLERANCE {
            return Err(not_a_line());
        }
        Ok(Axis {
            origin: point(start),
            direction: direction.scaled(1.0 / length),
        })
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

/// A plane or sketch face can use only flat faces (section 3.8).
fn is_flat(info: &ElementInfo) -> Result<bool, FeatureError> {
    Ok(info.fingerprint.kind == NameGeometry::Plane)
}

/// An edge axis can use only straight edges (section 3.8).
fn is_straight(info: &ElementInfo) -> Result<bool, FeatureError> {
    Ok(info.fingerprint.kind == NameGeometry::Line)
}

/// A face axis can use only faces with an axis (section 3.8).
fn has_axis(info: &ElementInfo) -> Result<bool, FeatureError> {
    Ok(matches!(
        info.fingerprint.kind,
        NameGeometry::Cylinder | NameGeometry::Cone | NameGeometry::Sphere | NameGeometry::Torus
    ))
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

/// The coordinates of a kernel point.
fn coordinates(point: Point) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// The face placement rule (section 3.6) on a face with this geometry: its plane through the face
/// centroid with the outward normal, origin and x-axis taken from `hint` as `Placement::on_plane`
/// does. None when the face is not flat.
fn on_face(
    geometry: &ElementGeometry,
    hint: &Placement,
) -> Result<Option<Placement>, FeatureError> {
    if geometry.kind != GeometryKind::Plane {
        return Ok(None);
    }
    let normal = geometry
        .direction
        .ok_or_else(|| FeatureError::internal("the kernel gave a plane without a normal"))?;
    Placement::on_plane(
        coordinates(geometry.centroid),
        [normal.x, normal.y, normal.z],
        hint,
    )
    .map(Some)
    .map_err(|error| FeatureError::internal(format!("placing a plane on a face failed: {error}")))
}

/// The canonical axis through `through` along `direction` (section 3.6).
fn canonical_axis(through: Point, direction: Vector) -> Result<Axis, FeatureError> {
    let unit = direction
        .normalized()
        .ok_or_else(|| FeatureError::internal("the kernel gave an axis without a direction"))?;
    let sign = [unit.x, unit.y, unit.z]
        .into_iter()
        .find(|component| component.abs() > AXIS_SIGN_TOLERANCE)
        .map_or(1.0, f64::signum);
    let unit = unit.scaled(sign);
    let along = through.x * unit.x + through.y * unit.y + through.z * unit.z;
    Ok(Axis {
        origin: Point::new(
            through.x - along * unit.x,
            through.y - along * unit.y,
            through.z - along * unit.z,
        ),
        direction: unit,
    })
}

#[cfg(test)]
mod tests {
    use partsmith_naming::{ElementName, Fingerprint, Motion, Successions};
    use partsmith_occt_sys::{
        Plane as KernelPlane, make_cone, make_cylinder, make_sphere, make_torus,
    };
    use partsmith_sketch::{SketchEntity, build_profile};
    use serde_json::json;

    use super::*;
    use crate::params::{OriginAxis, PlacementExt, ProfileRef};
    use crate::test_support::{Fixture, box_body, id, manifest, named_primitive, values};

    fn reference(query: &str, fingerprint: &str) -> Reference {
        Reference::new(
            ElementName::parse(query).expect("valid name"),
            Fingerprint::parse(fingerprint).expect("valid fingerprint"),
        )
        .expect("the kinds agree")
    }

    /// A far-away fingerprint that matches nothing.
    const LOST_FACE: &str = "plane|n=0,0,1|c=500,500,500|a=1";

    /// A fixture whose `b-1` is a 10 mm box with its corner at the origin.
    fn box_fixture() -> Fixture {
        Fixture::new("f-test1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
    }

    /// A fixture whose `b-1` is a 10 mm box with its corner at the origin whose faces have the
    /// fallback names of imported faces, `f-imp1/face#1` to `#6`, numbered by centroid x, then y,
    /// then z (naming.md sections 3.4 and 3.6.14), so the top is `#4`.
    fn imported_box_fixture() -> Fixture {
        let block = crate::test_support::named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let face = ElementName::parse("f-imp1/face").expect("valid name");
        let faces = (0..block.naming.count(ElementKind::Face))
            .map(|_| partsmith_naming::FaceSource::New(face.clone()))
            .collect();
        let named = crate::naming::name_shape(
            &block.shape,
            faces,
            &[],
            &Successions::default(),
            &Motion::IDENTITY,
        )
        .expect("the box is named");
        Fixture::new("f-test1")
            .with_body("b-1", BodyState::new(block.shape, Arc::new(named.naming)))
    }

    /// A fixture whose `b-1` is a cylinder of radius 5 and height 10 standing on XY at (3, 4).
    fn cylinder_fixture() -> Fixture {
        let placement = KernelPlane {
            origin: Point::new(3.0, 4.0, 0.0),
            ..KernelPlane::XY
        };
        let cylinder = named_primitive(
            "f-cyl1",
            make_cylinder(&placement, 5.0, 10.0).expect("the cylinder builds"),
        );
        Fixture::new("f-test1").with_body(
            "b-1",
            BodyState::new(cylinder.shape, Arc::new(cylinder.naming)),
        )
    }

    /// The output of a sketch on `placement` with these entities.
    fn sketch_output(placement: Placement, entities: serde_json::Value) -> Arc<FeatureOutput> {
        let entities: Vec<SketchEntity> =
            serde_json::from_value(entities).expect("the entities deserialize");
        let solved = serde_json::from_value(json!({
            "status": "under_constrained",
            "failed": false,
            "entities": [],
            "dof": 0,
            "redundant": [],
            "conflicting": [],
            "entity_states": {},
            "measured": {}
        }))
        .expect("the solve result deserializes");
        Arc::new(FeatureOutput {
            data: OutputData::Sketch(SketchResult {
                placement,
                motion: Motion::IDENTITY,
                profile: build_profile(&entities),
                solved: partsmith_sketch::SolveResult { entities, ..solved },
            }),
            warnings: Vec::new(),
            absent: Vec::new(),
        })
    }

    /// The rectangle (0, 0)-(80, 50) of lines e-1 to e-4, plus a circle e-5 of radius 10 at its center.
    fn rectangle_with_circle() -> serde_json::Value {
        json!([
            {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
            {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
            {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
            {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
            {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10},
            {"id": "e-9", "type": "line", "start": [0, 0], "end": [0, 10], "construction": true}
        ])
    }

    fn sketch_fixture() -> Fixture {
        let mut fixture = Fixture::new("f-ext1");
        fixture.upstream.insert(
            id("f-sk1"),
            sketch_output(OriginPlane::Xy.placement(), rectangle_with_circle()),
        );
        fixture
    }

    fn profile(regions: serde_json::Value) -> ProfileRef {
        serde_json::from_value(json!({"sketch": "f-sk1", "regions": regions}))
            .expect("the profile deserializes")
    }

    fn codes(warnings: &[FeatureWarning]) -> Vec<&'static str> {
        warnings.iter().map(FeatureWarning::code).collect()
    }

    /// A fixture whose `b-1` is the primitive `feature` made by `make` at `origin`.
    fn primitive_fixture(
        feature: &str,
        origin: Point,
        make: impl FnOnce(&KernelPlane) -> partsmith_occt_sys::Primitive,
    ) -> Fixture {
        let placement = KernelPlane {
            origin,
            ..KernelPlane::XY
        };
        let primitive = named_primitive(feature, make(&placement));
        Fixture::new("f-test1").with_body(
            "b-1",
            BodyState::new(primitive.shape, Arc::new(primitive.naming)),
        )
    }

    /// The axis of the face `face` of `b-1`.
    fn face_axis(fixture: &Fixture, face: &str, fingerprint: &str) -> Result<Axis, FeatureError> {
        fixture
            .context()
            .axis(
                "axis",
                &AxisRef::Face {
                    body: id("b-1"),
                    face: reference(face, fingerprint),
                    direction: None,
                },
            )
            .map(|axis| axis.value)
    }

    fn assert_axis(axis: &Axis, origin: [f64; 3], direction: [f64; 3]) {
        let actual = [axis.origin.x, axis.origin.y, axis.origin.z];
        for (component, expected) in actual.iter().zip(origin) {
            assert!((component - expected).abs() < 1e-9, "origin {actual:?}");
        }
        assert_eq!(axis.direction, vector(direction));
    }

    /// A fingerprint that six faces of a 10 mm box match equally: only the kind and the area.
    const ANY_BOX_FACE: &str = "plane|a=100";

    /// An imported face that is gone. Fallback names are confirmed by their fingerprint, so with
    /// `ANY_BOX_FACE` every face of `imported_box_fixture` fits it equally (naming.md section 3.9
    /// step 1); an unknown name of any other face never resolves by its fingerprint.
    const GONE_IMPORTED_FACE: &str = "f-imp1/face#9";

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
    fn a_sphere_axis_runs_along_z_through_its_center() {
        let fixture = primitive_fixture("f-sph1", Point::new(3.0, 4.0, 5.0), |placement| {
            make_sphere(placement, 2.0).expect("the sphere builds")
        });

        let axis = face_axis(&fixture, "f-sph1/surface", "sphere").expect("a sphere has an axis");

        assert_axis(&axis, [3.0, 4.0, 0.0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_cone_face_gives_its_axis() {
        let fixture = primitive_fixture("f-cone1", Point::new(3.0, 4.0, 0.0), |placement| {
            make_cone(placement, 5.0, 2.0, 10.0).expect("the cone builds")
        });

        let axis = face_axis(&fixture, "f-cone1/side", "cone").expect("a cone has an axis");

        assert_axis(&axis, [3.0, 4.0, 0.0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_torus_face_gives_its_axis() {
        let fixture = primitive_fixture("f-tor1", Point::new(3.0, 4.0, 2.0), |placement| {
            make_torus(placement, 10.0, 2.0).expect("the torus builds")
        });

        let axis = face_axis(&fixture, "f-tor1/surface", "torus").expect("a torus has an axis");

        assert_axis(&axis, [3.0, 4.0, 0.0], [0.0, 0.0, 1.0]);
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
    fn face_planes_use_the_face_placement_rule() {
        let fixture = box_fixture();
        let plane = PlaneRef::Face {
            body: id("b-1"),
            face: reference("f-box1/top", "plane|n=0,0,1"),
        };

        let placement = fixture
            .context()
            .plane("plane", &plane)
            .expect("the top is flat")
            .value;

        assert_eq!(placement.origin, [0.0, 0.0, 10.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(placement.normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_face_plane_on_a_side_face_projects_world_x() {
        let fixture = box_fixture();
        let plane = PlaneRef::Face {
            body: id("b-1"),
            face: reference("f-box1/front", "plane|n=0,-1,0"),
        };

        let placement = fixture
            .context()
            .plane("plane", &plane)
            .expect("the front is flat")
            .value;

        assert_eq!(placement.origin, [0.0, 0.0, 0.0]);
        assert_eq!(placement.x_axis, [1.0, 0.0, 0.0]);
        assert_eq!(placement.normal, [0.0, -1.0, 0.0]);
    }

    #[test]
    fn a_round_face_is_not_a_plane() {
        let fixture = cylinder_fixture();
        let plane = PlaneRef::Face {
            body: id("b-1"),
            face: reference("f-cyl1/side", "cylinder|n=0,0,1"),
        };

        let error = fixture
            .context()
            .plane("plane", &plane)
            .expect_err("a cylinder is round");

        assert!(matches!(error, FeatureError::NotPlanar { field } if field.path == "plane.face"));
    }

    /// A placement that is not an origin plane, so the face placement rule visibly uses it.
    fn tilted_hint() -> Placement {
        Placement {
            origin: [1.0, 2.0, 3.0],
            x_axis: [0.6, 0.0, 0.8],
            normal: [-0.8, 0.0, 0.6],
        }
    }

    fn face_placement_of(
        fixture: &Fixture,
        query: &str,
        fingerprint: &str,
    ) -> Result<Result<Placement, FallbackReason>, FeatureError> {
        fixture
            .context()
            .face_placement(&id("b-1"), &reference(query, fingerprint), &tilted_hint())
    }

    #[test]
    fn face_placement_projects_the_hint_onto_a_flat_face() {
        let placement = face_placement_of(&box_fixture(), "f-box1/top", "plane|n=0,0,1")
            .expect("nothing fails")
            .expect("the top is flat");

        let close = |actual: [f64; 3], expected: [f64; 3]| {
            actual
                .iter()
                .zip(expected)
                .all(|(value, wanted)| (value - wanted).abs() <= 1e-9)
        };
        assert!(close(placement.origin, [1.0, 2.0, 10.0]), "{placement:?}");
        assert!(close(placement.x_axis, [1.0, 0.0, 0.0]), "{placement:?}");
        assert!(close(placement.normal, [0.0, 0.0, 1.0]), "{placement:?}");
    }

    #[test]
    fn face_placement_of_a_lost_face_is_missing() {
        let result = face_placement_of(&box_fixture(), "f-box9/top", LOST_FACE);

        assert_eq!(result, Ok(Err(FallbackReason::Missing)));
    }

    #[test]
    fn face_placement_of_an_edge_reference_is_missing() {
        let result = face_placement_of(
            &box_fixture(),
            "edge(f-box1/front|f-box1/top)",
            "line|n=1,0,0",
        );

        assert_eq!(result, Ok(Err(FallbackReason::Missing)));
    }

    #[test]
    fn face_placement_of_an_ambiguous_face_is_ambiguous() {
        let result = face_placement_of(&imported_box_fixture(), GONE_IMPORTED_FACE, ANY_BOX_FACE);

        assert_eq!(result, Ok(Err(FallbackReason::Ambiguous)));
    }

    #[test]
    fn face_placement_of_a_round_face_is_not_planar() {
        let result = face_placement_of(&cylinder_fixture(), "f-cyl1/side", "cylinder|n=0,0,1");

        assert_eq!(result, Ok(Err(FallbackReason::NotPlanar)));
    }

    #[test]
    fn datum_planes_use_the_datum_placement() {
        let mut fixture = Fixture::new("f-test1");
        let placement = OriginPlane::Yz.placement().offset(4.0);
        fixture.upstream.insert(
            id("f-dp1"),
            Arc::new(FeatureOutput {
                data: OutputData::Datum(DatumResult::Plane(placement)),
                warnings: Vec::new(),
                absent: Vec::new(),
            }),
        );

        let resolved = fixture
            .context()
            .plane(
                "plane",
                &PlaneRef::Datum {
                    feature: id("f-dp1"),
                },
            )
            .expect("the datum is declared");

        assert_eq!(resolved.value, placement);
        assert!(resolved.warnings.is_empty());
    }

    #[test]
    fn a_sketch_as_a_datum_plane_names_the_feature_field() {
        let fixture = sketch_fixture();

        let error = fixture
            .context()
            .plane(
                "plane",
                &PlaneRef::Datum {
                    feature: id("f-sk1"),
                },
            )
            .expect_err("a sketch is no datum");

        assert!(matches!(
            error,
            FeatureError::WrongFeatureType { field, expected: Noun::DatumPlane, .. } if field.path == "plane.feature"
        ));
    }

    #[test]
    fn origin_axes_run_through_the_origin() {
        let fixture = Fixture::new("f-test1");

        let axis = fixture
            .context()
            .axis(
                "axis",
                &AxisRef::Origin {
                    axis: OriginAxis::Y,
                },
            )
            .expect("origin axes always resolve")
            .value;

        assert_eq!(axis.origin, Point::new(0.0, 0.0, 0.0));
        assert_eq!(axis.direction, Vector::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn straight_edges_give_canonical_axes() {
        let fixture = box_fixture();
        let axis = AxisRef::Edge {
            body: id("b-1"),
            edge: reference("edge(f-box1/back|f-box1/top)", "line|n=1,0,0"),
            direction: None,
        };

        let resolved = fixture
            .context()
            .axis("axis", &axis)
            .expect("the edge is straight")
            .value;

        assert_eq!(resolved.direction, Vector::new(1.0, 0.0, 0.0));
        assert_eq!(resolved.origin, Point::new(0.0, 10.0, 10.0));
    }

    #[test]
    fn round_edges_are_not_straight() {
        let fixture = cylinder_fixture();
        let axis = AxisRef::Edge {
            body: id("b-1"),
            edge: reference("edge(f-cyl1/side|f-cyl1/top)", "circle|n=0,0,1"),
            direction: None,
        };

        let error = fixture
            .context()
            .axis("axis", &axis)
            .expect_err("a circle is round");

        assert!(matches!(error, FeatureError::NotStraight { field } if field.path == "axis.edge"));
    }

    #[test]
    fn cylinder_faces_give_their_axis() {
        let fixture = cylinder_fixture();
        let axis = AxisRef::Face {
            body: id("b-1"),
            face: reference("f-cyl1/side", "cylinder|n=0,0,1"),
            direction: None,
        };

        let resolved = fixture
            .context()
            .axis("axis", &axis)
            .expect("a cylinder has an axis")
            .value;

        assert_eq!(resolved.direction, Vector::new(0.0, 0.0, 1.0));
        assert!((resolved.origin.x - 3.0).abs() < 1e-9);
        assert!((resolved.origin.y - 4.0).abs() < 1e-9);
        assert!(resolved.origin.z.abs() < 1e-9);
    }

    #[test]
    fn flat_faces_have_no_axis() {
        let fixture = box_fixture();
        let axis = AxisRef::Face {
            body: id("b-1"),
            face: reference("f-box1/top", "plane|n=0,0,1"),
            direction: None,
        };

        let error = fixture
            .context()
            .axis("axis", &axis)
            .expect_err("a plane has no axis");

        assert!(matches!(error, FeatureError::NoAxis { field } if field.path == "axis.face"));
    }

    #[test]
    fn sketch_lines_run_from_start_to_end_in_world_space() {
        let mut fixture = Fixture::new("f-rev1");
        fixture.upstream.insert(
            id("f-sk1"),
            sketch_output(OriginPlane::Xz.placement(), rectangle_with_circle()),
        );
        let axis = AxisRef::SketchLine {
            sketch: id("f-sk1"),
            line: id("e-9"),
        };

        let resolved = fixture
            .context()
            .axis("axis", &axis)
            .expect("e-9 is a line")
            .value;

        assert_eq!(resolved.origin, Point::new(0.0, 0.0, 0.0));
        assert_eq!(resolved.direction, Vector::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn sketch_axes_need_a_line_entity() {
        let fixture = sketch_fixture();
        let circle = AxisRef::SketchLine {
            sketch: id("f-sk1"),
            line: id("e-5"),
        };
        let unknown = AxisRef::SketchLine {
            sketch: id("f-sk1"),
            line: id("e-77"),
        };

        let not_a_line = fixture
            .context()
            .axis("axis", &circle)
            .expect_err("e-5 is a circle");
        let missing = fixture
            .context()
            .axis("axis", &unknown)
            .expect_err("no e-77");

        assert!(matches!(
            not_a_line,
            FeatureError::NotALine { ref field, .. } if field.path == "axis"
        ));
        assert!(matches!(
            missing,
            FeatureError::UnknownEntity { noun: Noun::Line, ref field, .. } if field.path == "axis"
        ));
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
    fn canonical_axes_point_their_first_component_up() {
        let axis = canonical_axis(Point::new(3.0, 4.0, 5.0), Vector::new(0.0, -2.0, 0.0))
            .expect("a direction with length");

        assert_eq!(axis.direction, Vector::new(0.0, 1.0, 0.0));
        assert_eq!(axis.origin, Point::new(3.0, 0.0, 5.0));
    }

    #[test]
    fn canonical_axes_ignore_tiny_components() {
        let axis = canonical_axis(Point::new(0.0, 0.0, 0.0), Vector::new(1e-12, 0.0, -1.0))
            .expect("a direction with length");

        assert_eq!(axis.direction.z, 1.0);
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
