//! Planes, axes and face placements resolved from their params (features.md section 3.6).

use partsmith_model::{Id, LINEAR_TOLERANCE};
use partsmith_naming::{ElementInfo, ElementKind, GeometryKind as NameGeometry, Reference};
use partsmith_occt_sys::{Axis, ElementGeometry, GeometryKind, Point, Vector};
use partsmith_sketch::{Geometry, OriginPlane, Placement};

use super::{BuildContext, Usable, WithWarnings};
use crate::error::{FallbackCause, FeatureError, Loss, Noun};
use crate::output::DatumResult;
use crate::params::{AxisRef, PlaneRef, point, vector};

/// Components at most this large count as zero when an axis direction is canonicalized (section 3.6).
const AXIS_SIGN_TOLERANCE: f64 = 1e-9;

impl BuildContext<'_> {
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
                let WithWarnings {
                    value: geometry,
                    warnings,
                } = self.first_element(&face_field, body, face, ElementKind::Face, &is_flat)?;
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
    /// (section 3.6): the face placement rule with `hint` in place of the XY placement, with the
    /// warnings and notices of the face reference at `field` (a guess, a follow, section 3.8). The
    /// inner `Err` says why the face cannot place it (an empty body, a missing or ambiguous
    /// reference with its loss, a face that is not flat), so the caller can fall back; the outer
    /// `Err` is for failures no fallback fixes, such as an undeclared body or a kernel error while
    /// reading the face.
    pub fn face_placement(
        &self,
        field: &str,
        body: &Id,
        face: &Reference,
        hint: &Placement,
    ) -> Result<Result<WithWarnings<Placement>, FaceFallback>, FeatureError> {
        let state = self.body(body)?;
        let Some(shape) = state.shape() else {
            return Ok(Err(FaceFallback::without_loss(FallbackCause::Missing)));
        };
        let resolved = match self.resolve_one(field, body, face, ElementKind::Face, &is_flat)? {
            Ok(resolved) => resolved,
            Err(loss) => return Ok(Err(FaceFallback::lost(loss))),
        };
        let first = resolved
            .elements
            .first()
            .ok_or_else(|| FeatureError::internal("a resolved face has no element"))?;
        let geometry = shape.element(first.kind, first.index)?;
        let Some(placement) = on_face(&geometry, hint)? else {
            return Ok(Err(FaceFallback::without_loss(FallbackCause::NotPlanar)));
        };
        Ok(Ok(WithWarnings {
            value: placement,
            warnings: resolved.into_messages(),
        }))
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
                let WithWarnings {
                    value: geometry,
                    warnings,
                } = self.first_element(&edge_field, body, edge, ElementKind::Edge, &is_straight)?;
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
                let WithWarnings {
                    value: geometry,
                    warnings,
                } = self.first_element(&face_field, body, face, ElementKind::Face, &has_axis)?;
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

    /// The geometry of the first element a required reference resolves to, with the reference's warnings and
    /// notices.
    fn first_element(
        &self,
        field: &str,
        body: &Id,
        reference: &Reference,
        kind: ElementKind,
        usable: &Usable<'_>,
    ) -> Result<WithWarnings<ElementGeometry>, FeatureError> {
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
        Ok(WithWarnings {
            value: shape.element(first.kind, first.index)?,
            warnings: resolved.into_messages(),
        })
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

/// Why a face cannot place a plane (section 3.6).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceFallback {
    /// The reason.
    pub cause: FallbackCause,
    /// The loss behind it: the resolver's loss for `Missing` and `Ambiguous` (none for an empty body), and for
    /// `NotCoplanar` an ambiguous loss whose candidates are the resolved faces; none for `NotPlanar`.
    pub loss: Option<Loss>,
}

impl FaceFallback {
    /// A fallback with no loss behind it: an empty body, or a face that is not flat.
    fn without_loss(cause: FallbackCause) -> FaceFallback {
        FaceFallback { cause, loss: None }
    }

    /// The fallback of a face reference that is missing or ambiguous, with its loss.
    fn lost(loss: Loss) -> FaceFallback {
        FaceFallback {
            cause: if loss.ambiguous {
                FallbackCause::Ambiguous
            } else {
                FallbackCause::Missing
            },
            loss: Some(loss),
        }
    }
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
    use std::sync::Arc;

    use partsmith_occt_sys::{
        Plane as KernelPlane, make_cone, make_cylinder, make_sphere, make_torus,
    };

    use super::*;
    use crate::context::BodyState;
    use crate::context::fixtures::{
        ANY_BOX_FACE, GONE_IMPORTED_FACE, LOST_FACE, box_fixture, imported_box_fixture,
        rectangle_with_circle, reference, sketch_fixture, sketch_output,
    };
    use crate::output::{FeatureOutput, OutputData};
    use crate::params::{OriginAxis, PlacementExt};
    use crate::test_support::{Fixture, id, named_primitive};

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
    ) -> Result<Result<WithWarnings<Placement>, FaceFallback>, FeatureError> {
        fixture.context().face_placement(
            "plane.face",
            &id("b-1"),
            &reference(query, fingerprint),
            &tilted_hint(),
        )
    }

    #[test]
    fn face_placement_projects_the_hint_onto_a_flat_face() {
        let placed = face_placement_of(&box_fixture(), "f-box1/top", "plane|n=0,0,1")
            .expect("nothing fails")
            .expect("the top is flat");
        let placement = placed.value;

        let close = |actual: [f64; 3], expected: [f64; 3]| {
            actual
                .iter()
                .zip(expected)
                .all(|(value, wanted)| (value - wanted).abs() <= 1e-9)
        };
        assert!(close(placement.origin, [1.0, 2.0, 10.0]), "{placement:?}");
        assert!(close(placement.x_axis, [1.0, 0.0, 0.0]), "{placement:?}");
        assert!(close(placement.normal, [0.0, 0.0, 1.0]), "{placement:?}");
        assert!(placed.warnings.is_empty(), "{:?}", placed.warnings);
    }

    /// The fallback of a face placement that could not use its face.
    fn fallback(
        result: Result<Result<WithWarnings<Placement>, FaceFallback>, FeatureError>,
    ) -> FaceFallback {
        match result {
            Ok(Err(fallback)) => fallback,
            other => panic!("expected a fallback, got {other:?}"),
        }
    }

    #[test]
    fn face_placement_of_a_lost_face_is_missing_with_its_loss() {
        let fallback = fallback(face_placement_of(&box_fixture(), "f-box9/top", LOST_FACE));

        assert_eq!(fallback.cause, FallbackCause::Missing);
        let loss = fallback.loss.expect("a lost reference has a loss");
        assert!(!loss.ambiguous);
        assert_eq!(loss.position, 0);
        assert_eq!(loss.reference.query.as_str(), "f-box9/top");
    }

    #[test]
    fn face_placement_of_an_edge_reference_is_missing() {
        let fallback = fallback(face_placement_of(
            &box_fixture(),
            "edge(f-box1/front|f-box1/top)",
            "line|n=1,0,0",
        ));

        assert_eq!(fallback.cause, FallbackCause::Missing);
        assert!(fallback.loss.is_some_and(|loss| !loss.ambiguous));
    }

    #[test]
    fn face_placement_of_an_ambiguous_face_is_ambiguous_with_its_loss() {
        let fallback = fallback(face_placement_of(
            &imported_box_fixture(),
            GONE_IMPORTED_FACE,
            ANY_BOX_FACE,
        ));

        assert_eq!(fallback.cause, FallbackCause::Ambiguous);
        assert!(fallback.loss.is_some_and(|loss| loss.ambiguous));
    }

    #[test]
    fn face_placement_of_a_round_face_is_not_planar_without_a_loss() {
        let fallback = fallback(face_placement_of(
            &cylinder_fixture(),
            "f-cyl1/side",
            "cylinder|n=0,0,1",
        ));

        assert_eq!(
            fallback,
            FaceFallback::without_loss(FallbackCause::NotPlanar)
        );
    }

    #[test]
    fn datum_planes_use_the_datum_placement() {
        let mut fixture = Fixture::new("f-test1");
        let placement = OriginPlane::Yz.placement().offset(4.0);
        fixture.upstream.insert(
            id("f-dp1"),
            Arc::new(FeatureOutput::new(
                OutputData::Datum(DatumResult::Plane(placement)),
                Vec::new(),
            )),
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
}
