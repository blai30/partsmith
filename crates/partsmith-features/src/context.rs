//! The read-only build context and what it is made from (features.md sections 2.6, 3.4 and 3.6 to 3.8).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use partsmith_model::{Dimension, Expr, FeatureManifest, FieldUnit, Id, Scope, Units};
use partsmith_naming::{ElementInfo, ElementKind, ElementName, NamingMap, Reference, Resolution};
use partsmith_occt_sys::{Axis, ElementGeometry, GeometryKind, Point, Shape, SubShape, Vector};
use partsmith_sketch::{Geometry, MatchMethod, OriginPlane, Placement, RegionMatch, match_region};

use crate::dependencies::ExprUse;
use crate::error::FeatureError;
use crate::field::FieldRef;
use crate::message::{Bound, Noun};
use crate::naming::{kernel, naming, sub_shape_kind, topology_info};
use crate::output::{DatumResult, FeatureOutput, OutputData, SketchResult};
use crate::params::{AxisRef, PlaneRef, point, vector};
use crate::warning::FeatureWarning;

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

    /// The declared whole number at `field`.
    pub fn eval_count(&self, field: &str, expr: &Expr) -> Result<u32, FeatureError> {
        let count = self.evaluated(field, expr, FieldUnit::Count)?;
        // `Values::evaluate` checked that the count is a whole number in the u32 range.
        Ok(count as u32)
    }

    /// State of a declared body just before this feature.
    pub fn body(&self, body: &Id) -> Result<&'a BodyState, FeatureError> {
        self.inputs
            .bodies
            .get(body)
            .ok_or_else(|| FeatureError::UndeclaredDependency {
                what: format!("body '{body}'"),
            })
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

    /// Resolves the regions of a profile, with warnings for remapped, ambiguous or missing regions (section 3.7).
    pub fn profile(
        &self,
        field: &str,
        profile: &crate::params::ProfileRef,
    ) -> Result<ResolvedProfile<'a>, FeatureError> {
        let sketch = self.sketch(&format!("{field}.sketch"), &profile.sketch)?;
        let mut regions = Vec::new();
        let mut warnings = Vec::new();
        for (position, region) in profile.regions.iter().enumerate() {
            let entry = || self.field(&format!("{field}.regions[{position}]"));
            match match_region(&sketch.profile.regions, region) {
                RegionMatch::Resolved { index, method } => {
                    if method != MatchMethod::Key {
                        warnings.push(FeatureWarning::RegionRemapped {
                            field: entry(),
                            position,
                            sketch: profile.sketch.clone(),
                            region: region.clone(),
                        });
                    }
                    if !regions.contains(&index) {
                        regions.push(index);
                    }
                }
                RegionMatch::Ambiguous { .. } => warnings.push(FeatureWarning::RegionAmbiguous {
                    field: entry(),
                    position,
                    sketch: profile.sketch.clone(),
                    region: region.clone(),
                }),
                RegionMatch::Missing => warnings.push(FeatureWarning::RegionMissing {
                    field: entry(),
                    position,
                    sketch: profile.sketch.clone(),
                    region: region.clone(),
                }),
            }
        }
        if regions.is_empty() {
            return Err(FeatureError::ProfileNotFound {
                field: self.field(field),
                sketch: profile.sketch.clone(),
            });
        }
        Ok(ResolvedProfile {
            sketch,
            regions,
            warnings,
        })
    }

    /// Resolves one required reference to its elements, in resolver order (section 3.8). Missing
    /// or ambiguous is `ReferenceNotFound`; an empty body is `BodyEmpty`.
    pub fn resolve(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
    ) -> Result<Vec<SubShape>, FeatureError> {
        let state = self.solid_body(body)?;
        let elements = state.elements()?;
        let not_found = |candidates| FeatureError::ReferenceNotFound {
            field: self.field(field),
            body: body.clone(),
            reference: reference.clone(),
            kind,
            candidates,
        };
        match lookup(state, &elements, reference, kind)? {
            Lookup::Found(found) => Ok(found),
            Lookup::Ambiguous(candidates) => Err(not_found(candidates)),
            Lookup::Missing => Err(not_found(Vec::new())),
        }
    }

    /// Resolves a list of references (section 3.8). Missing or ambiguous entries become warnings
    /// and are skipped; a non-empty list that resolves to nothing is `ReferencesNotFound`.
    pub fn resolve_all(
        &self,
        field: &str,
        body: &Id,
        references: &[Reference],
        kind: ElementKind,
    ) -> Result<Resolved, FeatureError> {
        let state = self.solid_body(body)?;
        let elements = state.elements()?;
        let mut resolved = Resolved {
            elements: Vec::new(),
            warnings: Vec::new(),
        };
        for (position, reference) in references.iter().enumerate() {
            let entry = || self.field(&format!("{field}[{position}]"));
            match lookup(state, &elements, reference, kind)? {
                Lookup::Found(found) => {
                    for element in found {
                        if !resolved.elements.contains(&element) {
                            resolved.elements.push(element);
                        }
                    }
                }
                Lookup::Ambiguous(candidates) => {
                    resolved.warnings.push(FeatureWarning::ReferenceAmbiguous {
                        field: entry(),
                        position,
                        body: body.clone(),
                        reference: reference.clone(),
                        kind,
                        candidates,
                    });
                }
                Lookup::Missing => resolved.warnings.push(FeatureWarning::ReferenceMissing {
                    field: entry(),
                    position,
                    body: body.clone(),
                    reference: reference.clone(),
                    kind,
                }),
            }
        }
        if !references.is_empty() && resolved.elements.is_empty() {
            return Err(FeatureError::ReferencesNotFound {
                field: self.field(field),
                count: references.len(),
                kind,
            });
        }
        Ok(resolved)
    }

    /// Resolves a plane to a placement (section 3.6). A face uses the face placement rule.
    pub fn plane(&self, field: &str, plane: &PlaneRef) -> Result<Placement, FeatureError> {
        match plane {
            PlaneRef::Origin { plane } => Ok(plane.placement()),
            PlaneRef::Datum { feature } => {
                match self.datum(&format!("{field}.feature"), feature)? {
                    DatumResult::Plane(placement) => Ok(*placement),
                }
            }
            PlaneRef::Face { body, face } => {
                let face_field = format!("{field}.face");
                let geometry = self.first_element(&face_field, body, face, ElementKind::Face)?;
                if geometry.kind != GeometryKind::Plane {
                    return Err(FeatureError::NotPlanar {
                        field: self.field(&face_field),
                    });
                }
                let normal = geometry.direction.ok_or_else(|| {
                    FeatureError::internal("the kernel gave a plane without a normal")
                })?;
                Placement::on_plane(
                    coordinates(geometry.centroid),
                    [normal.x, normal.y, normal.z],
                    &OriginPlane::Xy.placement(),
                )
                .map_err(|error| FeatureError::internal(error.to_string()))
            }
        }
    }

    /// Resolves an axis to a unit direction through a point (section 3.6). Edge and face axes are
    /// canonicalized: the first component above 1e-9 in absolute value is positive, and the origin
    /// is the point of the line closest to the world origin.
    pub fn axis(&self, field: &str, axis: &AxisRef) -> Result<Axis, FeatureError> {
        match axis {
            AxisRef::Origin { axis } => Ok(Axis {
                origin: Point::new(0.0, 0.0, 0.0),
                direction: vector(axis.direction()),
            }),
            AxisRef::SketchLine { sketch, line } => self.sketch_line(field, sketch, line),
            AxisRef::Edge { body, edge } => {
                let edge_field = format!("{field}.edge");
                let geometry = self.first_element(&edge_field, body, edge, ElementKind::Edge)?;
                match (geometry.kind, geometry.direction) {
                    (GeometryKind::Line, Some(direction)) => {
                        canonical_axis(geometry.centroid, direction)
                    }
                    _ => Err(FeatureError::NotStraight {
                        field: self.field(&edge_field),
                    }),
                }
            }
            AxisRef::Face { body, face } => {
                let face_field = format!("{field}.face");
                let geometry = self.first_element(&face_field, body, face, ElementKind::Face)?;
                let direction = match geometry.kind {
                    GeometryKind::Cylinder | GeometryKind::Cone | GeometryKind::Torus => {
                        geometry.direction
                    }
                    GeometryKind::Sphere => Some(Vector::new(0.0, 0.0, 1.0)),
                    _ => None,
                };
                match (geometry.location, direction) {
                    (Some(location), Some(direction)) => canonical_axis(location, direction),
                    _ => Err(FeatureError::NoAxis {
                        field: self.field(&face_field),
                    }),
                }
            }
        }
    }

    /// True once the run has been canceled. Long builds check it between steps.
    pub fn is_canceled(&self) -> bool {
        self.inputs.cancel.is_canceled()
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

    /// A declared body that holds a solid; an empty one is `BodyEmpty` for a modifying feature.
    fn solid_body(&self, body: &Id) -> Result<&'a BodyState, FeatureError> {
        let state = self.body(body)?;
        if state.is_empty() {
            return Err(FeatureError::BodyEmpty {
                body: body.clone(),
                use_new: false,
            });
        }
        Ok(state)
    }

    /// The geometry of the first element a required reference resolves to.
    fn first_element(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
    ) -> Result<ElementGeometry, FeatureError> {
        let found = self.resolve(field, body, reference, kind)?;
        let first = found
            .first()
            .ok_or_else(|| FeatureError::internal("a resolved reference has no element"))?;
        let shape = self
            .solid_body(body)?
            .shape()
            .ok_or_else(|| FeatureError::internal("a body with a solid has no shape"))?;
        shape.element(first.kind, first.index).map_err(kernel)
    }

    /// The axis of a sketch line: from its solved start to its end, in world space.
    fn sketch_line(&self, field: &str, sketch: &Id, line: &Id) -> Result<Axis, FeatureError> {
        let result = self.sketch(&format!("{field}.sketch"), sketch)?;
        let line_field = format!("{field}.line");
        let entity = result
            .solved
            .entities
            .iter()
            .find(|entity| entity.id == *line)
            .ok_or_else(|| FeatureError::UnknownEntity {
                field: self.field(&line_field),
                sketch: sketch.clone(),
                entity: line.to_string(),
                noun: Noun::Line,
            })?;
        let not_a_line = || FeatureError::NotALine {
            field: self.field(&line_field),
            sketch: sketch.clone(),
            entity: line.clone(),
        };
        let Geometry::Line { start, end } = entity.geometry else {
            return Err(not_a_line());
        };
        let start = result.to_world(start);
        let end = result.to_world(end);
        let direction = Vector::new(end[0] - start[0], end[1] - start[1], end[2] - start[2]);
        if direction.length() < partsmith_model::LINEAR_TOLERANCE {
            return Err(not_a_line());
        }
        let unit = direction.normalized().ok_or_else(not_a_line)?;
        Ok(Axis {
            origin: point(start),
            direction: unit,
        })
    }
}

/// Evaluated values of one feature's declared expressions.
#[derive(Clone, Debug, Default)]
pub struct Values {
    /// The uses in declaration order, each with its converted value or error.
    entries: Vec<(ExprUse, Result<f64, FeatureError>)>,
    /// Position in `entries` of the first use of each field.
    by_field: BTreeMap<String, usize>,
}

impl Values {
    /// Evaluates each use with `expr.evaluate(scope, Some(expected))` and converts it (section 3.4).
    pub fn evaluate(uses: &[ExprUse], scope: &Scope<'_>, manifest: &FeatureManifest) -> Values {
        let mut values = Values::default();
        for declared in uses {
            let value = evaluate_use(declared, scope, manifest);
            values
                .by_field
                .entry(declared.field.clone())
                .or_insert(values.entries.len());
            values.entries.push((declared.clone(), value));
        }
        values
    }

    /// Every use with its converted value or error, in declaration order. Recompute hashes them into cache keys (recompute.md section 3.4).
    pub fn iter(&self) -> impl Iterator<Item = (&ExprUse, &Result<f64, FeatureError>)> + '_ {
        self.entries
            .iter()
            .map(|(declared, value)| (declared, value))
    }

    /// The first use declared at `field` with its value.
    fn get(&self, field: &str) -> Option<&(ExprUse, Result<f64, FeatureError>)> {
        self.by_field
            .get(field)
            .and_then(|&position| self.entries.get(position))
    }
}

/// The state of a body at one point in the timeline. Cloning is cheap.
#[derive(Clone, Debug)]
pub struct BodyState {
    /// The shared state.
    content: Arc<BodyContent>,
}

/// What a `BodyState` shares between its clones.
#[derive(Debug)]
struct BodyContent {
    /// The solid, or None for an empty body.
    shape: Option<Shape>,
    /// Names of the shape's elements.
    naming: Arc<NamingMap>,
    /// Element infos, computed on first use.
    elements: OnceLock<Result<Arc<[ElementInfo]>, FeatureError>>,
}

impl BodyState {
    /// A body with no solid.
    pub fn empty() -> Self {
        let naming = NamingMap::build(
            Vec::new(),
            &partsmith_naming::TopologyInfo {
                faces: Vec::new(),
                edges: Vec::new(),
                vertices: Vec::new(),
            },
            &[],
        )
        .expect("a shape without elements always has a naming map");
        BodyState::with(None, Arc::new(naming))
    }

    /// A body holding `shape` named by `naming`.
    pub fn new(shape: Shape, naming: Arc<NamingMap>) -> Self {
        BodyState::with(Some(shape), naming)
    }

    /// The shape, or None when the body is empty.
    pub fn shape(&self) -> Option<&Shape> {
        self.content.shape.as_ref()
    }

    /// The naming map of the shape (empty for an empty body).
    pub fn naming(&self) -> &Arc<NamingMap> {
        &self.content.naming
    }

    /// True when the body has no solid.
    pub fn is_empty(&self) -> bool {
        self.content.shape.is_none()
    }

    /// Element infos (names, kinds, fingerprints, adjacency) of the shape, computed once on first use and shared by clones.
    pub fn elements(&self) -> Result<Arc<[ElementInfo]>, FeatureError> {
        self.content
            .elements
            .get_or_init(|| match &self.content.shape {
                None => Ok(Arc::from([])),
                Some(shape) => {
                    let topology = topology_info(shape)?;
                    let infos = self
                        .content
                        .naming
                        .element_infos(&topology)
                        .map_err(naming)?;
                    Ok(Arc::from(infos))
                }
            })
            .clone()
    }

    /// A state with this shape and naming map.
    fn with(shape: Option<Shape>, naming: Arc<NamingMap>) -> Self {
        BodyState {
            content: Arc::new(BodyContent {
                shape,
                naming,
                elements: OnceLock::new(),
            }),
        }
    }
}

/// Result of resolving a list of references.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// Resolved sub-shapes (kernel indices) in reference order, then resolver order, without duplicates.
    pub elements: Vec<SubShape>,
    /// One warning per skipped reference.
    pub warnings: Vec<FeatureWarning>,
}

/// Result of resolving a profile.
#[derive(Clone, Debug)]
pub struct ResolvedProfile<'a> {
    /// The sketch result.
    pub sketch: &'a SketchResult,
    /// Indices into the sketch result's regions, in profile order, without duplicates.
    pub regions: Vec<usize>,
    /// Warnings for remapped, ambiguous or missing regions.
    pub warnings: Vec<FeatureWarning>,
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

/// What one reference denotes in a body state.
enum Lookup {
    /// These elements, in resolver order.
    Found(Vec<SubShape>),
    /// Several candidates fit equally well.
    Ambiguous(Vec<ElementName>),
    /// Nothing fits, or the reference selects another kind of element.
    Missing,
}

/// Resolves one reference of `kind` against a body state's elements.
fn lookup(
    state: &BodyState,
    elements: &[ElementInfo],
    reference: &Reference,
    kind: ElementKind,
) -> Result<Lookup, FeatureError> {
    if reference.query.kind() != kind {
        return Ok(Lookup::Missing);
    }
    match partsmith_naming::resolve(reference, elements, state.naming().aliases()) {
        Resolution::Resolved { elements, .. } => elements
            .iter()
            .map(|name| {
                let index = state.naming().index(name).ok_or_else(|| {
                    FeatureError::internal(format!("the resolved element {name} has no index"))
                })?;
                Ok(SubShape {
                    kind: sub_shape_kind(kind),
                    index,
                })
            })
            .collect::<Result<_, FeatureError>>()
            .map(Lookup::Found),
        Resolution::Ambiguous { candidates } => Ok(Lookup::Ambiguous(
            candidates
                .into_iter()
                .map(|candidate| candidate.name)
                .collect(),
        )),
        Resolution::Missing { .. } => Ok(Lookup::Missing),
    }
}

/// Evaluates one declared use and converts it (section 3.4).
fn evaluate_use(
    declared: &ExprUse,
    scope: &Scope<'_>,
    manifest: &FeatureManifest,
) -> Result<f64, FeatureError> {
    let expected = match declared.kind {
        FieldUnit::Length => Dimension::LENGTH,
        FieldUnit::Angle => Dimension::ANGLE,
        FieldUnit::Scalar | FieldUnit::Count => Dimension::NONE,
    };
    let field = || FieldRef::new(&declared.field, manifest);
    let quantity = declared
        .expr
        .evaluate(scope, Some(expected))
        .map_err(|error| FeatureError::Expression {
            field: field(),
            expr: declared.expr.clone(),
            error,
        })?;
    match declared.kind {
        FieldUnit::Count => {
            whole_number(quantity.value).map_err(|problem| problem.error(field(), quantity.value))
        }
        FieldUnit::Length | FieldUnit::Angle | FieldUnit::Scalar => Ok(quantity.value),
    }
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

/// The nearest whole number to `value` when it is within 1e-9 of it and in the u32 range. The
/// range is checked first, so a negative or huge fraction reports the range it misses.
fn whole_number(value: f64) -> Result<f64, CountProblem> {
    let nearest = value.round();
    let whole = (value - nearest).abs() <= WHOLE_NUMBER_TOLERANCE;
    let checked = if whole { nearest } else { value };
    if checked < 0.0 {
        return Err(CountProblem::OutOfRange(Bound::AtLeast(0.0)));
    }
    if checked > f64::from(u32::MAX) {
        return Err(CountProblem::OutOfRange(Bound::AtMost(f64::from(u32::MAX))));
    }
    if !whole {
        return Err(CountProblem::NotWhole);
    }
    // `round` keeps the sign of -0.0; counts are written without it.
    Ok(nearest.abs())
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
    use partsmith_model::{Category, EmptyContext, ParameterValues};

    use super::*;

    fn manifest() -> FeatureManifest {
        FeatureManifest {
            label: "Test".to_string(),
            category: Category::Create,
            icon: "test".to_string(),
            version: 1,
            fields: Vec::new(),
        }
    }

    fn values(uses: &[(&str, &str, FieldUnit)]) -> Values {
        let uses: Vec<ExprUse> = uses
            .iter()
            .map(|(field, expr, kind)| ExprUse {
                field: field.to_string(),
                expr: Expr::new(*expr),
                kind: *kind,
            })
            .collect();
        let parameters = ParameterValues::default();
        let scope = Scope {
            units: Units::Mm,
            parameters: &parameters,
            context: &EmptyContext,
        };
        Values::evaluate(&uses, &scope, &manifest())
    }

    #[test]
    fn whole_numbers_within_tolerance() {
        assert!(matches!(whole_number(2.9999999999), Ok(value) if value == 3.0));
        assert!(
            matches!(whole_number(-0.0000000001), Ok(value) if value == 0.0 && value.is_sign_positive())
        );
        assert!(whole_number(4294967295.0).is_ok());
    }

    #[test]
    fn negative_fractions_report_the_range() {
        assert!(matches!(
            whole_number(-2.5),
            Err(CountProblem::OutOfRange(Bound::AtLeast(_)))
        ));
    }

    #[test]
    fn fractions_are_not_whole() {
        assert!(matches!(whole_number(2.5), Err(CountProblem::NotWhole)));
    }

    #[test]
    fn values_keep_declaration_order() {
        let values = values(&[("b", "2", FieldUnit::Length), ("a", "1", FieldUnit::Scalar)]);

        let fields: Vec<&str> = values
            .iter()
            .map(|(declared, _)| declared.field.as_str())
            .collect();
        assert_eq!(fields, ["b", "a"]);
    }

    #[test]
    fn scalars_are_plain_numbers() {
        let values = values(&[("factor", "1.5", FieldUnit::Scalar)]);

        let (_, value) = values.iter().next().expect("one use");
        assert_eq!(value, &Ok(1.5));
    }

    #[test]
    fn a_repeated_field_reads_its_first_use() {
        let values = values(&[("a", "1", FieldUnit::Scalar), ("a", "2", FieldUnit::Scalar)]);

        let (declared, _) = values.get("a").expect("a is declared");
        assert_eq!(declared.expr.as_str(), "1");
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
    fn empty_bodies_have_no_elements() {
        let state = BodyState::empty();

        assert!(state.is_empty());
        assert!(state.shape().is_none());
        assert_eq!(state.elements().expect("no elements").len(), 0);
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
