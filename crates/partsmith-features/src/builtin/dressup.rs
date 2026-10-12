//! Helpers shared by the dress-up features `fillet`, `chamfer` and `shell` (features/fillet.md,
//! features/chamfer.md and features/shell.md): edge selection with the smooth-edge rule, the
//! kernel call with the "too large" search, the origins and names of result faces, and the
//! outputs.

use std::sync::Arc;

use partsmith_model::Id;
use partsmith_naming::{
    ElementKind, ElementName, FaceSource, Generator, NamingMap, Reference, roles,
};
use partsmith_occt_sys::{
    EdgeTopology, ElementGeometry, GeometryKind, InputHistory, KernelError, KernelErrorKind,
    OpResult, Point, Shape, SubShapeKind, Topology, Vector,
};

use super::common::{bodies, unchanged};
use crate::{
    BodyChange, BodyState, BodyUpdate, BuildContext, FeatureError, FeatureOutput, FeatureWarning,
    IgnoredReason, name_shape, too_large_search,
};

/// Largest angle in radians between the outward normals of an edge's two faces at the edge's
/// midpoint for which the faces meet without a corner (fillet.md section 3.3 step 4).
const SMOOTH_ANGLE: f64 = 1e-6;

/// A kernel failure as a feature error.
pub(super) fn kernel(error: KernelError) -> FeatureError {
    FeatureError::Kernel { error }
}

/// A failure that valid kernel output cannot cause.
pub(super) fn internal(message: impl Into<String>) -> FeatureError {
    FeatureError::Internal {
        message: message.into(),
    }
}

/// The edges a fillet or chamfer works on.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EdgeSelection {
    /// Kernel indices of the selected edges that have a corner, in resolver order.
    pub(super) edges: Vec<u32>,
    /// Warnings for lost or ambiguous references, then one per skipped smooth edge.
    pub(super) warnings: Vec<FeatureWarning>,
}

/// Resolves the `edges` references in `body`, whose shape is `shape`, and skips the smooth edges
/// with an `Ignored` warning each (fillet.md section 3.3 steps 3 and 4, chamfer.md step 3).
pub(super) fn select_edges(
    context: &BuildContext<'_>,
    body: &Id,
    shape: &Shape,
    edges: &[Reference],
) -> Result<EdgeSelection, FeatureError> {
    let resolved = context.resolve_all("edges", body, edges, ElementKind::Edge)?;
    let topology = shape.topology().map_err(kernel)?;
    let mut selection = EdgeSelection {
        edges: Vec::new(),
        warnings: resolved.warnings,
    };
    for element in resolved.elements {
        if is_smooth(shape, &topology, element.index)? {
            selection.warnings.push(FeatureWarning::Ignored {
                field: context.field("edges"),
                reason: IgnoredReason::SmoothEdge,
            });
        } else {
            selection.edges.push(element.index);
        }
    }
    Ok(selection)
}

/// True when `edge` of `shape` is a seam, or when its two faces meet without a corner: the angle
/// between their outward normals at the edge's midpoint is at most `SMOOTH_ANGLE`.
///
/// The kernel describes faces by their surface, not by normals at a point, so the normals come
/// from that description: flat faces give their outward normal, cylinders and spheres their
/// normal up to its sign. An edge whose midpoint or face normals cannot be found that way (an
/// edge that is not a line or circle, or a face on another kind of surface) counts as having a
/// corner, and the kernel decides whether it can be blended.
pub(super) fn is_smooth(
    shape: &Shape,
    topology: &Topology,
    edge: u32,
) -> Result<bool, FeatureError> {
    let entry = edge
        .checked_sub(1)
        .and_then(|position| topology.edges.get(position as usize))
        .ok_or_else(|| internal(format!("edge {edge} is not in the body's topology")))?;
    if entry.seam || entry.faces.len() == 1 {
        return Ok(true);
    }
    let [first, second] = entry.faces[..] else {
        return Ok(false);
    };
    let Some(midpoint) = edge_midpoint(shape, entry, edge)? else {
        return Ok(false);
    };
    let face = |index| shape.element(SubShapeKind::Face, index).map_err(kernel);
    let normals = (
        face_normal(&face(first)?, midpoint),
        face_normal(&face(second)?, midpoint),
    );
    Ok(match normals {
        (Some(first), Some(second)) => angle_between(first, second) <= SMOOTH_ANGLE,
        _ => false,
    })
}

/// The midpoint of a straight or circular edge: a line's length centroid, the middle of an arc,
/// or the vertex of a full circle (every point of which is alike). None for other curves.
fn edge_midpoint(
    shape: &Shape,
    entry: &EdgeTopology,
    edge: u32,
) -> Result<Option<Point>, FeatureError> {
    let geometry = shape.element(SubShapeKind::Edge, edge).map_err(kernel)?;
    let (GeometryKind::Circle, Some(center), Some(radius)) =
        (geometry.kind, geometry.location, geometry.radius)
    else {
        return Ok((geometry.kind == GeometryKind::Line).then_some(geometry.centroid));
    };
    if let [vertex] = entry.vertices[..] {
        let position = shape
            .element(SubShapeKind::Vertex, vertex)
            .map_err(kernel)?
            .centroid;
        return Ok(Some(position));
    }
    // The length centroid of an arc lies on the ray from the center through the arc's middle.
    Ok(difference(geometry.centroid, center)
        .normalized()
        .map(|direction| center.offset(direction.scaled(radius))))
}

/// A face's unit normal at a point. The kernel gives the outward sign of flat faces only, so the
/// normal of a curved face is known up to its sign.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Normal {
    /// The unit normal.
    direction: Vector,
    /// True when `direction` points out of the material.
    outward: bool,
}

/// The normal of `face` at `point` (a point on the face): the outward normal of a flat face, the
/// radial direction of a cylinder or sphere. None for other surfaces.
fn face_normal(face: &ElementGeometry, point: Point) -> Option<Normal> {
    let unoriented = |direction: Vector| {
        direction.normalized().map(|direction| Normal {
            direction,
            outward: false,
        })
    };
    match face.kind {
        GeometryKind::Plane => face.direction.map(|direction| Normal {
            direction,
            outward: true,
        }),
        GeometryKind::Cylinder => {
            let axis = face.direction?;
            let offset = difference(point, face.location?);
            unoriented(subtract(offset, axis.scaled(offset.dot(axis))))
        }
        GeometryKind::Sphere => unoriented(difference(point, face.location?)),
        _ => None,
    }
}

/// The angle between two normals in [0, pi]; between their lines in [0, pi / 2] when the sign of
/// either is unknown.
fn angle_between(first: Normal, second: Normal) -> f64 {
    let cross = first.direction.cross(second.direction).length();
    let dot = first.direction.dot(second.direction);
    let dot = if first.outward && second.outward {
        dot
    } else {
        dot.abs()
    };
    cross.atan2(dot)
}

/// The vector from `from` to `to`.
fn difference(to: Point, from: Point) -> Vector {
    Vector::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

/// `first - second`.
fn subtract(first: Vector, second: Vector) -> Vector {
    Vector::new(first.x - second.x, first.y - second.y, first.z - second.z)
}

/// Runs a fillet, chamfer or shell `operation` at the `requested` value. When the kernel fails for
/// any reason other than invalid input, searches for the largest value that works (features.md
/// section 4.3) and returns the error `too_large` makes from that value and the first failure.
/// Invalid input is reported as the kernel's error.
pub(super) fn run_or_search<T>(
    context: &BuildContext<'_>,
    requested: f64,
    operation: impl Fn(f64) -> Result<T, KernelError>,
    too_large: impl FnOnce(Option<f64>, KernelError) -> FeatureError,
) -> Result<T, FeatureError> {
    if context.is_canceled() {
        return Err(FeatureError::Canceled);
    }
    match operation(requested) {
        Ok(result) => Ok(result),
        Err(error) if error.kind == KernelErrorKind::InvalidInput => Err(kernel(error)),
        Err(cause) => {
            let max = too_large_search(context, requested, |value| operation(value).is_ok())?;
            Err(too_large(max, cause))
        }
    }
}

/// Where one face of a fillet, chamfer or shell result comes from, as elements of the input body
/// (naming.md section 3.6: `modified_sources` and `generators`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct FaceOrigins {
    /// Body faces whose `modified` list holds the face: it is that face, kept or trimmed.
    pub(super) modified: Vec<u32>,
    /// Body faces whose `generated` list holds the face.
    pub(super) faces: Vec<u32>,
    /// Body edges whose `generated` list holds the face.
    pub(super) edges: Vec<u32>,
    /// Body vertices whose `generated` list holds the face.
    pub(super) vertices: Vec<u32>,
}

/// Adds the history of one kernel input to `origins`, which has one entry per face of the final
/// shape. `body_index` maps an element of the kernel input to its index in the body, and
/// `final_face` maps a face of the kernel result to its index in the final shape.
pub(super) fn add_origins(
    origins: &mut [FaceOrigins],
    history: &InputHistory,
    body_index: impl Fn(SubShapeKind, u32) -> Option<u32>,
    final_face: impl Fn(u32) -> Option<u32>,
) -> Result<(), FeatureError> {
    let face_count = origins.len();
    let position = |face: u32| {
        final_face(face)
            .and_then(|index| index.checked_sub(1))
            .map(|position| position as usize)
            .filter(|&position| position < face_count)
            .ok_or_else(|| {
                internal(format!(
                    "the kernel history names result face {face}, which is not one of the {face_count} faces"
                ))
            })
    };
    let kinds = [
        (SubShapeKind::Face, &history.faces),
        (SubShapeKind::Edge, &history.edges),
        (SubShapeKind::Vertex, &history.vertices),
    ];
    for (kind, fates) in kinds {
        for (input, fate) in (1_u32..).zip(fates) {
            let body = body_index(kind, input).ok_or_else(|| {
                internal(format!(
                    "the kernel history names {kind:?} {input}, which the body does not have"
                ))
            })?;
            if kind == SubShapeKind::Face {
                for &face in &fate.modified {
                    origins[position(face)?].modified.push(body);
                }
            }
            for generated in &fate.generated {
                if generated.kind != SubShapeKind::Face {
                    continue;
                }
                let origin = &mut origins[position(generated.index)?];
                match kind {
                    SubShapeKind::Face => origin.faces.push(body),
                    SubShapeKind::Edge => origin.edges.push(body),
                    SubShapeKind::Vertex => origin.vertices.push(body),
                    SubShapeKind::Solid => {}
                }
            }
        }
    }
    Ok(())
}

/// One entry per face of a kernel result whose only input is the body, with that input's history.
pub(super) fn single_input_origins(result: &OpResult) -> Result<Vec<FaceOrigins>, FeatureError> {
    let [history] = &result.history.inputs[..] else {
        return Err(internal("a fillet or chamfer result has one history input"));
    };
    let mut origins = vec![FaceOrigins::default(); result.shape.count(SubShapeKind::Face) as usize];
    add_origins(&mut origins, history, |_, index| Some(index), Some)?;
    Ok(origins)
}

/// The names of body faces, which are always named.
pub(super) fn face_names(
    naming: &NamingMap,
    faces: &[u32],
) -> Result<Vec<ElementName>, FeatureError> {
    faces
        .iter()
        .map(|&face| {
            naming
                .name(ElementKind::Face, face)
                .cloned()
                .ok_or_else(|| internal(format!("face {face} of the body has no name")))
        })
        .collect()
}

/// The smallest name in byte order of these body elements of `kind`; unnamed elements (such as
/// degenerated edges) are left out.
pub(super) fn smallest_name(
    naming: &NamingMap,
    kind: ElementKind,
    elements: &[u32],
) -> Option<ElementName> {
    elements
        .iter()
        .filter_map(|&element| naming.name(kind, element))
        .min()
        .cloned()
}

/// The source of a face the feature created: `<feature>/<role>`, or `<feature>/<role>/<element>`
/// when it was made from an input element.
pub(super) fn new_face(
    feature: &Id,
    role: &str,
    element: Option<&ElementName>,
) -> Result<FaceSource, FeatureError> {
    ElementName::generated(feature, role, element.map(Generator::Element))
        .map(FaceSource::New)
        .map_err(|error| internal(format!("naming the result failed: {error}")))
}

/// Names a fillet or chamfer result (naming.md sections 3.6.5 and 3.6.6). Each face keeps the
/// names of the body faces it descends from; a face generated from body edges is named
/// `<feature>/<role>/<smallest edge name>`, one generated from body vertices
/// `<feature>/corner/<smallest vertex name>`, and any other face falls back to `<feature>/face`.
pub(super) fn name_blend(
    feature: &Id,
    role: &str,
    input: &NamingMap,
    result: &OpResult,
) -> Result<NamingMap, FeatureError> {
    let faces = single_input_origins(result)?
        .iter()
        .map(|origin| {
            if !origin.modified.is_empty() {
                return Ok(FaceSource::From(face_names(input, &origin.modified)?));
            }
            if let Some(edge) = smallest_name(input, ElementKind::Edge, &origin.edges) {
                return new_face(feature, role, Some(&edge));
            }
            if let Some(vertex) = smallest_name(input, ElementKind::Vertex, &origin.vertices) {
                return new_face(feature, roles::CORNER, Some(&vertex));
            }
            new_face(feature, roles::FACE, None)
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    name_shape(&result.shape, faces, &[input.aliases()])
}

/// The output of a dress-up feature that changed `body` into `shape` named by `naming`.
pub(super) fn modified(
    body: &Id,
    shape: Shape,
    naming: NamingMap,
    warnings: Vec<FeatureWarning>,
) -> FeatureOutput {
    let update = BodyUpdate {
        body: body.clone(),
        shape: Some(shape),
        naming: Arc::new(naming),
        change: BodyChange::Modify,
    };
    bodies(vec![update], warnings)
}

/// The output of a fillet or chamfer left with no edge to work on: `body` keeps its `state`, and a
/// `NoEffect` warning follows the other warnings (fillet.md section 3.3 step 4).
pub(super) fn no_effect(
    body: &Id,
    state: &BodyState,
    mut warnings: Vec<FeatureWarning>,
) -> FeatureOutput {
    warnings.push(FeatureWarning::NoEffect { body: body.clone() });
    bodies(vec![unchanged(body, state, BodyChange::Modify)], warnings)
}

/// Fixtures shared by the dress-up unit tests.
#[cfg(test)]
pub(super) mod test_fixtures {
    use partsmith_model::{Id, Units};
    use partsmith_naming::Reference;
    use partsmith_sketch::RegionRef;

    use crate::{BodyOp, PythonNames};

    /// Script names with millimeter units, `b-1` as `block` and the parameter `wall` as `wall`.
    pub(in crate::builtin) struct ScriptNames;

    impl PythonNames for ScriptNames {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, body: &Id) -> Option<String> {
            (body.as_str() == "b-1").then(|| "block".to_string())
        }

        fn feature(&self, _feature: &Id) -> Option<String> {
            None
        }

        fn entity(&self, _sketch: &Id, _entity: &Id) -> Option<String> {
            None
        }

        fn parameter(&self, name: &str) -> Option<String> {
            (name == "wall").then(|| "wall".to_string())
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::Join
        }
    }

    /// A reference parsed from its JSON form.
    pub(in crate::builtin) fn reference(json: &str) -> Reference {
        serde_json::from_str(json).expect("references written in tests are valid")
    }
}

#[cfg(test)]
mod tests {
    use partsmith_occt_sys::{EdgeRadius, ElementFate, Plane, SubShape, fillet, make_cylinder};

    use super::*;
    use crate::test_support::{Fixture, face_names as sorted_face_names, id, named_box};

    /// The edges of `shape` whose faces are exactly these two.
    fn edge_between(shape: &Shape, first: u32, second: u32) -> u32 {
        let mut wanted = [first, second];
        wanted.sort_unstable();
        let topology = shape.topology().expect("topology");
        let position = topology
            .edges
            .iter()
            .position(|edge| edge.faces == wanted)
            .expect("the faces share an edge");
        u32::try_from(position + 1).expect("few edges")
    }

    /// The index of the face named `name`.
    fn face_index(naming: &NamingMap, name: &str) -> u32 {
        naming
            .index(&ElementName::parse(name).expect("valid name"))
            .expect("the face exists")
    }

    /// A 10 mm cube from the origin with its front top edge filleted at radius 2, and the index
    /// of the fillet face.
    fn filleted_cube() -> (Shape, u32) {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let edge = edge_between(
            &cube.shape,
            face_index(&cube.naming, "f-box1/front"),
            face_index(&cube.naming, "f-box1/top"),
        );
        let result = fillet(&cube.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("fillets");
        let faces = result.shape.elements(SubShapeKind::Face).expect("faces");
        let position = faces
            .iter()
            .position(|face| face.kind == GeometryKind::Cylinder)
            .expect("the fillet face is a cylinder");
        (
            result.shape,
            u32::try_from(position + 1).expect("few faces"),
        )
    }

    fn normal(x: f64, y: f64, z: f64, outward: bool) -> Normal {
        Normal {
            direction: Vector::new(x, y, z),
            outward,
        }
    }

    fn kernel_error(kind: KernelErrorKind) -> KernelError {
        KernelError {
            operation: partsmith_occt_sys::Operation::Fillet,
            kind,
            exception: "StdFail_NotDone".to_string(),
            message: String::new(),
        }
    }

    #[test]
    fn every_edge_of_a_box_has_a_corner() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let topology = cube.shape.topology().expect("topology");

        for edge in 1..=12 {
            assert!(
                !is_smooth(&cube.shape, &topology, edge).expect("checked"),
                "edge {edge}"
            );
        }
    }

    #[test]
    fn edges_where_a_fillet_meets_its_faces_are_smooth() {
        let (shape, fillet_face) = filleted_cube();
        let topology = shape.topology().expect("topology");

        let tangent: Vec<u32> = (1_u32..)
            .zip(&topology.edges)
            .filter(|(_, edge)| edge.faces.contains(&fillet_face))
            .filter(|(index, _)| {
                shape
                    .element(SubShapeKind::Edge, *index)
                    .expect("edge")
                    .kind
                    == GeometryKind::Line
            })
            .map(|(index, _)| index)
            .collect();

        assert_eq!(tangent.len(), 2, "the fillet meets the top and the front");
        for edge in tangent {
            assert!(
                is_smooth(&shape, &topology, edge).expect("checked"),
                "edge {edge}"
            );
        }
    }

    #[test]
    fn the_arc_ends_of_a_fillet_have_corners() {
        let (shape, fillet_face) = filleted_cube();
        let topology = shape.topology().expect("topology");

        let arcs: Vec<u32> = (1_u32..)
            .zip(&topology.edges)
            .filter(|(_, edge)| edge.faces.contains(&fillet_face))
            .filter(|(index, _)| {
                shape
                    .element(SubShapeKind::Edge, *index)
                    .expect("edge")
                    .kind
                    == GeometryKind::Circle
            })
            .map(|(index, _)| index)
            .collect();

        assert_eq!(arcs.len(), 2, "the fillet ends at the left and right faces");
        for edge in arcs {
            assert!(
                !is_smooth(&shape, &topology, edge).expect("checked"),
                "edge {edge}"
            );
        }
    }

    #[test]
    fn a_cylinder_seam_is_smooth_and_its_rims_have_corners() {
        let cylinder = make_cylinder(&Plane::XY, 5.0, 10.0)
            .expect("cylinder")
            .shape;
        let topology = cylinder.topology().expect("topology");

        let seams: Vec<bool> = topology.edges.iter().map(|edge| edge.seam).collect();
        for (edge, seam) in (1_u32..).zip(seams) {
            assert_eq!(
                is_smooth(&cylinder, &topology, edge).expect("checked"),
                seam,
                "edge {edge}"
            );
        }
    }

    #[test]
    fn the_midpoint_of_an_arc_lies_on_its_middle() {
        let (shape, fillet_face) = filleted_cube();
        let topology = shape.topology().expect("topology");
        let (index, entry) = (1_u32..)
            .zip(&topology.edges)
            .find(|(index, edge)| {
                edge.faces.contains(&fillet_face)
                    && shape
                        .element(SubShapeKind::Edge, *index)
                        .expect("edge")
                        .kind
                        == GeometryKind::Circle
            })
            .expect("the fillet has an arc edge");

        let midpoint = edge_midpoint(&shape, entry, index)
            .expect("measured")
            .expect("an arc has a midpoint");

        // The quarter arc of radius 2 around (x, 2, 8) runs from (x, 0, 8) to (x, 2, 10).
        let offset = 2.0 - 2.0 * std::f64::consts::FRAC_1_SQRT_2;
        assert!((midpoint.y - offset).abs() < 1e-9, "{midpoint:?}");
        assert!((midpoint.z - (10.0 - offset)).abs() < 1e-9, "{midpoint:?}");
    }

    #[test]
    fn oriented_normals_compare_with_their_signs() {
        let up = normal(0.0, 0.0, 1.0, true);
        let down = normal(0.0, 0.0, -1.0, true);

        assert_eq!(angle_between(up, up), 0.0);
        assert!((angle_between(up, down) - std::f64::consts::PI).abs() < 1e-12);
        assert!(
            (angle_between(up, normal(1.0, 0.0, 0.0, true)) - std::f64::consts::FRAC_PI_2).abs()
                < 1e-12
        );
    }

    #[test]
    fn a_normal_without_its_sign_compares_as_a_line() {
        let up = normal(0.0, 0.0, 1.0, true);

        assert_eq!(angle_between(up, normal(0.0, 0.0, -1.0, false)), 0.0);
    }

    #[test]
    fn invalid_input_is_reported_without_a_search() {
        let fixture = Fixture::new("f-fil1");
        let attempts = std::cell::Cell::new(0);

        let error = run_or_search(
            &fixture.context(),
            3.0,
            |_| {
                attempts.set(attempts.get() + 1);
                Err::<(), _>(kernel_error(KernelErrorKind::InvalidInput))
            },
            |_, _| internal("not expected"),
        )
        .expect_err("invalid input fails");

        assert_eq!(error.code(), "kernel");
        assert_eq!(attempts.get(), 1);
    }

    #[test]
    fn a_kernel_failure_searches_for_the_largest_working_value() {
        let fixture = Fixture::new("f-fil1");
        let largest = std::cell::Cell::new(None);

        let error = run_or_search(
            &fixture.context(),
            3.0,
            |value| {
                if value <= 2.0 {
                    Ok(())
                } else {
                    Err(kernel_error(KernelErrorKind::Failed))
                }
            },
            |max, cause| {
                largest.set(max);
                kernel(cause)
            },
        )
        .expect_err("the requested value fails");

        assert_eq!(error, kernel(kernel_error(KernelErrorKind::Failed)));
        let max = largest.get().expect("some value works");
        assert!(max <= 2.0 && max > 1.99, "{max}");
    }

    #[test]
    fn a_working_value_is_returned() {
        let fixture = Fixture::new("f-fil1");

        let value = run_or_search(&fixture.context(), 3.0, Ok, |_, _| internal("not expected"))
            .expect("works");

        assert_eq!(value, 3.0);
    }

    #[test]
    fn origins_follow_modified_and_generated_lists() {
        let history = InputHistory {
            faces: vec![
                ElementFate {
                    modified: vec![2],
                    generated: vec![SubShape {
                        kind: SubShapeKind::Face,
                        index: 3,
                    }],
                },
                ElementFate::default(),
            ],
            edges: vec![ElementFate {
                modified: Vec::new(),
                generated: vec![
                    SubShape {
                        kind: SubShapeKind::Face,
                        index: 1,
                    },
                    SubShape {
                        kind: SubShapeKind::Edge,
                        index: 4,
                    },
                ],
            }],
            vertices: vec![ElementFate {
                modified: Vec::new(),
                generated: vec![SubShape {
                    kind: SubShapeKind::Face,
                    index: 1,
                }],
            }],
        };
        let mut origins = vec![FaceOrigins::default(); 3];

        add_origins(&mut origins, &history, |_, index| Some(index + 10), Some)
            .expect("the history fits");

        assert_eq!(
            origins,
            [
                FaceOrigins {
                    edges: vec![11],
                    vertices: vec![11],
                    ..FaceOrigins::default()
                },
                FaceOrigins {
                    modified: vec![11],
                    ..FaceOrigins::default()
                },
                FaceOrigins {
                    faces: vec![11],
                    ..FaceOrigins::default()
                },
            ]
        );
    }

    #[test]
    fn origins_of_a_face_outside_the_result_are_internal() {
        let history = InputHistory {
            faces: vec![ElementFate {
                modified: vec![5],
                generated: Vec::new(),
            }],
            edges: Vec::new(),
            vertices: Vec::new(),
        };
        let mut origins = vec![FaceOrigins::default(); 2];

        let error = add_origins(&mut origins, &history, |_, index| Some(index), Some)
            .expect_err("face 5 does not exist");

        assert_eq!(error.code(), "internal");
    }

    #[test]
    fn a_filleted_edge_names_its_fillet_face() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let edge = edge_between(
            &cube.shape,
            face_index(&cube.naming, "f-box1/front"),
            face_index(&cube.naming, "f-box1/top"),
        );
        let result = fillet(&cube.shape, &[EdgeRadius { edge, radius: 2.0 }]).expect("fillets");

        let naming = name_blend(&id("f-fil1"), roles::FILLET, &cube.naming, &result)
            .expect("the result is named");

        assert_eq!(
            sorted_face_names(&naming),
            [
                "f-box1/back",
                "f-box1/bottom",
                "f-box1/front",
                "f-box1/left",
                "f-box1/right",
                "f-box1/top",
                "f-fil1/fillet/edge(f-box1/front|f-box1/top)",
            ]
        );
    }

    #[test]
    fn three_fillets_at_a_corner_name_the_corner_blend_after_the_vertex() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let edges: Vec<EdgeRadius> = (1..=12)
            .map(|edge| EdgeRadius { edge, radius: 1.0 })
            .collect();
        let result = fillet(&cube.shape, &edges).expect("fillets");

        let naming = name_blend(&id("f-fil1"), roles::FILLET, &cube.naming, &result)
            .expect("the result is named");

        let names = sorted_face_names(&naming);
        let corners = names
            .iter()
            .filter(|name| name.starts_with("f-fil1/corner/vertex("))
            .count();
        let fillets = names
            .iter()
            .filter(|name| name.starts_with("f-fil1/fillet/edge("))
            .count();
        assert_eq!((corners, fillets, names.len()), (8, 12, 26));
    }

    #[test]
    fn the_smallest_name_skips_unnamed_elements() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let smallest = smallest_name(&cube.naming, ElementKind::Face, &[1, 2, 3, 4, 5, 6, 99]);

        assert_eq!(smallest.expect("named faces").as_str(), "f-box1/back");
    }

    #[test]
    fn an_unchanged_body_keeps_its_state_and_warns() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let state = BodyState::new(cube.shape, Arc::new(cube.naming));

        let output = no_effect(&id("b-1"), &state, Vec::new());

        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect { body: id("b-1") }]
        );
        let crate::OutputData::Bodies(updates) = output.data else {
            panic!("a body update");
        };
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].change, BodyChange::Modify);
        assert!(Arc::ptr_eq(&updates[0].naming, state.naming()));
    }
}
