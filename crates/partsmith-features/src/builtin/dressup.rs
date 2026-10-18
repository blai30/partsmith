//! Helpers shared by the dress-up features `fillet`, `chamfer` and `shell` (features/fillet.md,
//! features/chamfer.md and features/shell.md): edge selection with the smooth-edge rule, the
//! kernel call with the "too large" search, the origins and names of result faces, and the
//! outputs.

use std::sync::Arc;

use partsmith_model::{Id, LINEAR_TOLERANCE};
use partsmith_naming::{
    EdgeInfo, ElementInfo, ElementKind, ElementName, FaceSource, Generator, NamingMap, Reference,
    TopologyInfo, roles,
};
use partsmith_occt_sys::{
    Continuity, EdgeNormals, InputHistory, KernelError, KernelErrorKind, OpResult, Shape,
    SubShapeKind,
};

use super::common::{angle_between, bodies, check_canceled, face_name, name_on_body, unchanged};
use crate::{
    BodyChange, BodyState, BodyUpdate, BuildContext, FeatureError, FeatureOutput, FeatureWarning,
    IgnoredReason, Limit, Measurements, Named, NoEffectHow, too_large_search,
};

/// Largest angle in radians between the outward normals of an edge's two faces at the edge's
/// middle for which the faces meet without a corner when the shape records no tangency between
/// them (fillet.md section 3.3 step 4).
const SMOOTH_ANGLE: f64 = 1e-3;

/// The edges a fillet or chamfer works on.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EdgeSelection {
    /// The selected edges that have a corner, in resolver order.
    pub(super) edges: Vec<SelectedEdge>,
    /// Warnings for lost, guessed or widened references, notices for followed ones, then one
    /// warning per skipped smooth edge.
    pub(super) warnings: Vec<FeatureWarning>,
}

/// One selected edge of the body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectedEdge {
    /// The edge's kernel index.
    pub(super) index: u32,
    /// Kernel indices of the faces next to the edge, ascending.
    pub(super) faces: Vec<u32>,
    /// The position in `edges` of the reference it was first resolved from (`Resolved::sources`), whose first face
    /// term gives a two-distance chamfer its side (chamfer.md section 3.3 step 4).
    pub(super) source: usize,
    /// The positions in `edges` of every reference that resolved to it (`Resolved::all_sources`), `source` first.
    pub(super) entries: Vec<usize>,
}

/// Resolves the `edges` references in `body`, which holds `shape` in `state`, and skips the smooth
/// edges with an `Ignored` warning each (fillet.md section 3.3 steps 3 and 4, chamfer.md step 3).
/// Smooth edges are never offered as candidates of a loss (features.md section 3.8). The faces of
/// each edge come from the body state's topology, which resolving the references has already read.
pub(super) fn select_edges(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    shape: &Shape,
    edges: &[Reference],
) -> Result<EdgeSelection, FeatureError> {
    let topology = state.topology()?;
    let has_corner = |info: &ElementInfo| match state.naming().index(&info.name) {
        Some(index) => Ok(!is_smooth(shape, &topology, index)?),
        None => Ok(false),
    };
    let resolved =
        context.resolve_all_usable("edges", body, edges, ElementKind::Edge, &has_corner)?;
    let mut selected = Vec::new();
    let mut skipped = Vec::new();
    for (position, (edge, &source)) in resolved.elements.iter().zip(&resolved.sources).enumerate() {
        if is_smooth(shape, &topology, edge.index)? {
            let field = match alone_entry(&resolved.all_sources, position) {
                Some(entry) => format!("edges[{entry}]"),
                None => "edges".to_string(),
            };
            skipped.push(FeatureWarning::Ignored {
                field: context.field(&field),
                reason: IgnoredReason::SmoothEdge,
            });
        } else {
            selected.push(SelectedEdge {
                index: edge.index,
                faces: edge_topology(&topology, edge.index)?.faces.clone(),
                source,
                entries: resolved.all_sources[position].clone(),
            });
        }
    }
    let mut warnings = resolved.into_messages();
    warnings.extend(skipped);
    Ok(EdgeSelection {
        edges: selected,
        warnings,
    })
}

/// The entry of the element at `position` of a resolved list (`Resolved::all_sources`) that its fix removes: the only
/// entry that resolved to it, when that entry resolved to it alone, so removing it leaves out that element and
/// nothing else (features.md section 4.2). None when another entry selects the element too, or the entry also
/// selects other elements, so the warning is on the whole list.
fn alone_entry(all_sources: &[Vec<usize>], position: usize) -> Option<usize> {
    let [entry] = all_sources[position][..] else {
        return None;
    };
    all_sources
        .iter()
        .enumerate()
        .all(|(other, entries)| other == position || !entries.contains(&entry))
        .then_some(entry)
}

/// The adjacency of `edge` (a 1-based kernel index) in `topology`.
fn edge_topology(topology: &TopologyInfo, edge: u32) -> Result<&EdgeInfo, FeatureError> {
    edge.checked_sub(1)
        .and_then(|position| topology.edges.get(position as usize))
        .ok_or_else(|| FeatureError::internal(format!("edge {edge} is not in the body's topology")))
}

/// True when `edge` of `shape` is smooth (fillet.md section 3.3 step 4): `topology` lists fewer
/// than two faces for it (a seam, or a degenerated edge such as a sphere pole), or its two faces
/// meet without a corner. An edge of three or more faces has a corner.
fn is_smooth(shape: &Shape, topology: &TopologyInfo, edge: u32) -> Result<bool, FeatureError> {
    match edge_topology(topology, edge)?.faces.len() {
        0 | 1 => Ok(true),
        2 => meets_without_corner(edge, shape.edge_normals(edge)),
        _ => Ok(false),
    }
}

/// Whether the two faces of `edge` meet without a corner, from the kernel's description of the
/// edge at its middle (`Shape::edge_normals`): the shape records them as tangent (`G1OrBetter`),
/// or their outward normals there make an angle of at most `SMOOTH_ANGLE`. Opposite normals (a
/// knife edge) are a corner, and so is an edge the kernel rejects as invalid input, which happens
/// where a face has no defined normal; any other kernel failure is reported.
fn meets_without_corner(
    edge: u32,
    normals: Result<EdgeNormals, KernelError>,
) -> Result<bool, FeatureError> {
    let normals = match normals {
        Ok(normals) => normals,
        Err(error) if error.kind == KernelErrorKind::InvalidInput => return Ok(false),
        Err(error) => return Err(FeatureError::from(error)),
    };
    let [first, second] = normals.faces[..] else {
        return Err(FeatureError::internal(format!(
            "the kernel describes edge {edge} with {} faces instead of 2",
            normals.faces.len()
        )));
    };
    Ok(normals.continuity == Some(Continuity::G1OrBetter)
        || angle_between(first.normal, second.normal) <= SMOOTH_ANGLE)
}

/// Runs a fillet, chamfer or shell `operation` at the `requested` value. When the kernel fails for
/// any reason other than invalid input or an internal error, searches for the largest value that
/// works (features.md section 4.3) and returns the error `too_large` makes from that value and the
/// first failure, measuring what limits it on this failure path only. Invalid input and internal
/// errors are reported as the kernel's error: the input was checked first, so they are not about
/// size.
pub(super) fn run_or_search<T>(
    context: &BuildContext<'_>,
    requested: f64,
    operation: impl Fn(f64) -> Result<T, KernelError>,
    too_large: impl FnOnce(Option<f64>, KernelError) -> Result<FeatureError, FeatureError>,
) -> Result<T, FeatureError> {
    check_canceled(context)?;
    match operation(requested) {
        Ok(result) => Ok(result),
        Err(error)
            if matches!(
                error.kind,
                KernelErrorKind::InvalidInput | KernelErrorKind::Internal
            ) =>
        {
            Err(FeatureError::from(error))
        }
        Err(cause) => {
            let max = too_large_search(context, requested, |value| operation(value).is_ok())?;
            Err(too_large(max, cause)?)
        }
    }
}

/// The narrowest face next to a selected edge of a fillet or chamfer, measured on its failure path
/// (features.md section 4.3), when it is narrower than `requested`: the kernel's `width_across` of
/// each face of each of `edges`, which come in name order, the faces of an edge in name order too,
/// so ties go to the edge and then the face whose names sort first. When the limiting edge comes
/// from one entry alone and another reference resolved, `works_without` tries the operation at
/// `requested` without the edges only that entry selects (`keep` false for them, in the order of
/// `edges`), which decides `removable`; an edge that another entry selects too stays after the
/// removal, so it is not removable.
pub(super) fn face_limit(
    state: &BodyState,
    shape: &Shape,
    edges: &[&SelectedEdge],
    requested: f64,
    works_without: impl FnOnce(&[bool]) -> bool,
) -> Result<Option<Limit>, FeatureError> {
    let naming = state.naming();
    let mut narrowest: Option<(f64, &SelectedEdge, u32)> = None;
    for &edge in edges {
        let mut faces: Vec<(&ElementName, u32)> = edge
            .faces
            .iter()
            .filter_map(|&face| Some((naming.name(ElementKind::Face, face)?, face)))
            .collect();
        faces.sort();
        for (_, face) in faces {
            // A face the kernel cannot measure across the edge states no limit.
            let Ok(width) = shape.width_across(face, edge.index) else {
                continue;
            };
            if narrowest.is_none_or(|(least, _, _)| width < least - LINEAR_TOLERANCE) {
                narrowest = Some((width, edge, face));
            }
        }
    }
    let Some((width, edge, face)) = narrowest.filter(|(width, _, _)| *width < requested) else {
        return Ok(None);
    };
    let removable = match edge.entries[..] {
        [entry] => {
            let keep: Vec<bool> = edges
                .iter()
                .map(|other| other.entries.iter().any(|&from| from != entry))
                .collect();
            keep.contains(&true) && works_without(&keep)
        }
        _ => false,
    };
    Ok(Some(Limit::Face {
        position: edge.source,
        edge: picked(state, ElementKind::Edge, edge.index)?,
        face: picked(state, ElementKind::Face, face)?,
        width,
        removable,
    }))
}

/// The items of `items` whose entry of `keep` is true, for an attempt without some edges.
pub(super) fn kept_edges<T: Copy>(items: &[T], keep: &[bool]) -> Vec<T> {
    items
        .iter()
        .zip(keep)
        .filter(|(_, kept)| **kept)
        .map(|(item, _)| *item)
        .collect()
}

/// The element of `kind` at kernel `index` of `state` as a pick stores it (`Reference::from_element`).
fn picked(state: &BodyState, kind: ElementKind, index: u32) -> Result<Reference, FeatureError> {
    let set = state.element_set()?;
    state
        .naming()
        .name(kind, index)
        .and_then(|name| set.element(name))
        .map(|info| Reference::from_element(info, &set))
        .ok_or_else(|| FeatureError::internal(format!("the {kind} {index} has no element")))
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
                FeatureError::internal(format!(
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
                FeatureError::internal(format!(
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
                    SubShapeKind::Solid | SubShapeKind::Shell => {}
                }
            }
        }
    }
    Ok(())
}

/// One entry per face of a kernel result whose only input is the body, with that input's history.
pub(super) fn single_input_origins(result: &OpResult) -> Result<Vec<FaceOrigins>, FeatureError> {
    let [history] = &result.history.inputs[..] else {
        return Err(FeatureError::internal(
            "a fillet or chamfer result has one history input",
        ));
    };
    let mut origins = vec![FaceOrigins::default(); result.shape.count(SubShapeKind::Face) as usize];
    add_origins(&mut origins, history, |_, index| Some(index), Some)?;
    Ok(origins)
}

/// The names of body faces, which are always named.
pub(super) fn names_of_faces(
    naming: &NamingMap,
    faces: &[u32],
) -> Result<Vec<ElementName>, FeatureError> {
    faces
        .iter()
        .map(|&face| {
            naming
                .name(ElementKind::Face, face)
                .cloned()
                .ok_or_else(|| {
                    FeatureError::internal(format!("face {face} of the body has no name"))
                })
        })
        .collect()
}

/// The source of a face that descends from these faces of the body, whose naming map is the only
/// inherited one.
pub(super) fn from_body(names: Vec<ElementName>) -> FaceSource {
    FaceSource::From(names.into_iter().map(|name| (0, name)).collect())
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
    face_name(feature, role, element.map(Generator::Element)).map(FaceSource::New)
}

/// The output of a fillet or chamfer that changed `body`, whose state before the feature is
/// `state`, into `result`: the result's faces named by `role` (`name_blend`), then the update.
pub(super) fn finish_blend(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    role: &str,
    result: OpResult,
    warnings: Vec<FeatureWarning>,
) -> Result<FeatureOutput, FeatureError> {
    let named = name_blend(context.feature(), role, state.naming(), &result)?;
    Ok(modified(body, result.shape, named, warnings))
}

/// Names a fillet or chamfer result (naming.md sections 3.6.5 and 3.6.6). Each face keeps the
/// names of the body faces it descends from; a face generated from body edges is named
/// `<feature>/<role>/<smallest edge name>`, one generated from body vertices
/// `<feature>/corner/<smallest vertex name>`, and any other face falls back to `<feature>/face`.
/// The new faces take the body's base motion (features.md section 3.9).
fn name_blend(
    feature: &Id,
    role: &str,
    input: &NamingMap,
    result: &OpResult,
) -> Result<Named, FeatureError> {
    let faces = single_input_origins(result)?
        .iter()
        .map(|origin| {
            if !origin.modified.is_empty() {
                return Ok(from_body(names_of_faces(input, &origin.modified)?));
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
    name_on_body(feature, &result.shape, faces, input)
}

/// The output of a dress-up feature that changed `body` into `shape` named by `named`, with the
/// topology that naming read.
pub(super) fn modified(
    body: &Id,
    shape: Shape,
    named: Named,
    warnings: Vec<FeatureWarning>,
) -> FeatureOutput {
    let update = BodyUpdate {
        body: body.clone(),
        shape: Some(shape),
        naming: Arc::new(named.naming),
        change: BodyChange::Modify,
        measured: Measurements {
            topology: Some(named.topology),
            volume: None,
        },
    };
    bodies(vec![update], warnings)
}

/// The selected `edges` sorted by their element names in byte order, the order the kernel gets
/// them in (fillet.md section 3.3 step 5, chamfer.md section 3.3 step 4), so the order of the
/// `edges` references never changes the result: the kernel's result depends on the order it gets
/// edges in (in the last bits of its geometry, and for a chamfer the side of a whole tangent
/// chain comes from the first of its edges it receives).
pub(super) fn sorted_by_name<'a>(
    naming: &NamingMap,
    edges: &'a [SelectedEdge],
) -> Result<Vec<&'a SelectedEdge>, FeatureError> {
    let mut named = edges
        .iter()
        .map(|edge| {
            naming
                .name(ElementKind::Edge, edge.index)
                .map(|name| (name, edge))
                .ok_or_else(|| {
                    FeatureError::internal(format!("the selected edge {} has no name", edge.index))
                })
        })
        .collect::<Result<Vec<_>, FeatureError>>()?;
    named.sort_by_key(|(name, _)| *name);
    Ok(named.into_iter().map(|(_, edge)| edge).collect())
}

/// The output of a fillet or chamfer left with no edge to work on: `body` keeps its `state`, and
/// when the `edges` field holds two or more `entries` a `NoEffect` warning follows the other
/// warnings; a single skipped edge is reported by its `Ignored` warning alone (fillet.md section
/// 3.3 step 4, features.md section 3.15).
pub(super) fn no_effect(
    context: &BuildContext<'_>,
    body: &Id,
    state: &BodyState,
    entries: usize,
    mut warnings: Vec<FeatureWarning>,
) -> FeatureOutput {
    if entries >= 2 {
        warnings.push(FeatureWarning::NoEffect {
            body: body.clone(),
            how: NoEffectHow::Skipped {
                field: context.field("edges"),
            },
            reverse_hits: false,
            attach: None,
        });
    }
    bodies(vec![unchanged(body, state, BodyChange::Modify)], warnings)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use partsmith_occt_sys::{
        EdgeRadius, ElementFate, FaceNormal, GeometryKind, Operation, PlanarCurve, PlanarLoop,
        PlanarRegion, Plane, Point, Point2, SubShape, TaggedCurve, Vector, extrude, fillet,
        make_cylinder, make_profile,
    };

    use super::*;
    use crate::test_support::{Fixture, box_body, face_names, id, named_box, reference};
    use crate::topology_info;

    #[test]
    fn edges_are_sorted_by_name_in_byte_order() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let index = |name: &str| {
            cube.naming
                .index(&ElementName::parse(name).expect("valid"))
                .expect("named")
        };
        let names = [
            "edge(f-box1/front|f-box1/top)",
            "edge(f-box1/back|f-box1/top)",
            "edge(f-box1/bottom|f-box1/front)",
        ];
        let edges: Vec<SelectedEdge> = names
            .iter()
            .map(|name| SelectedEdge {
                index: index(name),
                faces: Vec::new(),
                source: 0,
                entries: vec![0],
            })
            .collect();

        let sorted = sorted_by_name(&cube.naming, &edges).expect("every edge is named");

        let sorted: Vec<u32> = sorted.iter().map(|edge| edge.index).collect();
        assert_eq!(sorted, [index(names[1]), index(names[2]), index(names[0])]);
    }

    #[test]
    fn an_unnamed_edge_cannot_be_sorted() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let edge = SelectedEdge {
            index: 99,
            faces: Vec::new(),
            source: 0,
            entries: vec![0],
        };

        let error = sorted_by_name(&cube.naming, &[edge]).expect_err("edge 99 has no name");

        assert_eq!(error.code(), "internal");
    }

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

    /// A cylinder of radius 10 and height 20 with its top rim filleted at radius 2 (kernel.md
    /// test 97), and the index of each face by surface kind.
    fn filleted_rim() -> (Shape, impl Fn(GeometryKind) -> Vec<u32>) {
        let cylinder = make_cylinder(&Plane::XY, 10.0, 20.0)
            .expect("cylinder")
            .shape;
        let topology = cylinder.topology().expect("topology");
        let rim = (1_u32..)
            .zip(&topology.edges)
            .find(|(index, edge)| {
                edge.faces.len() == 2
                    && cylinder
                        .element(SubShapeKind::Edge, *index)
                        .expect("edge")
                        .centroid
                        .z
                        > 19.0
            })
            .map(|(index, _)| index)
            .expect("the top rim");
        let shape = fillet(
            &cylinder,
            &[EdgeRadius {
                edge: rim,
                radius: 2.0,
            }],
        )
        .expect("fillets")
        .shape;
        let kinds: Vec<GeometryKind> = shape
            .elements(SubShapeKind::Face)
            .expect("faces")
            .iter()
            .map(|face| face.kind)
            .collect();
        let faces_of = move |kind: GeometryKind| -> Vec<u32> {
            (1_u32..)
                .zip(&kinds)
                .filter(|(_, face)| **face == kind)
                .map(|(index, _)| index)
                .collect()
        };
        (shape, faces_of)
    }

    /// A 10 mm prism over the square [0, 10] x [0, 10] minus the half disc of radius 5 around
    /// (0, 5), whose arc touches the bottom and top sides at the left corners, and the index of
    /// the edge along the Z axis, where the flat face y = 0 meets the cylinder with the material
    /// between them coming to a knife edge.
    fn knife_edge_prism() -> (Shape, u32) {
        let point = |[x, y]: [f64; 2]| Point2 { x, y };
        let line = |tag, start, end| TaggedCurve {
            tag,
            curve: PlanarCurve::Line {
                start: point(start),
                end: point(end),
            },
        };
        let arc = TaggedCurve {
            tag: 4,
            curve: PlanarCurve::Arc {
                center: point([0.0, 5.0]),
                radius: 5.0,
                start: point([0.0, 0.0]),
                end: point([0.0, 10.0]),
            },
        };
        let region = PlanarRegion {
            outer: PlanarLoop {
                curves: vec![
                    line(1, [0.0, 0.0], [10.0, 0.0]),
                    line(2, [10.0, 0.0], [10.0, 10.0]),
                    line(3, [10.0, 10.0], [0.0, 10.0]),
                    arc,
                ],
            },
            holes: Vec::new(),
        };
        let profile = make_profile(&Plane::XY, &[region]).expect("the profile is valid");
        let shape = extrude(&profile, Vector::new(0.0, 0.0, 1.0), 0.0, 10.0)
            .expect("extrudes")
            .shape;
        let edges = shape.elements(SubShapeKind::Edge).expect("edges");
        let knife = (1_u32..)
            .zip(&edges)
            .find(|(_, edge)| {
                edge.kind == GeometryKind::Line
                    && edge.centroid.x.abs() < 1e-9
                    && edge.centroid.y.abs() < 1e-9
            })
            .map(|(index, _)| index)
            .expect("the edge along the Z axis");
        (shape, knife)
    }

    /// Kernel edge normals of an edge between two faces with these outward normals.
    fn two_faces(first: Vector, second: Vector) -> EdgeNormals {
        EdgeNormals {
            point: Point::new(0.0, 0.0, 0.0),
            tangent: Vector::new(1.0, 0.0, 0.0),
            faces: vec![
                FaceNormal {
                    face: 1,
                    normal: first,
                },
                FaceNormal {
                    face: 2,
                    normal: second,
                },
            ],
            continuity: Some(Continuity::C0),
        }
    }

    /// The unit vector at `angle` radians from the Z axis, toward the Y axis.
    fn tilted(angle: f64) -> Vector {
        Vector::new(0.0, angle.sin(), angle.cos())
    }

    fn kernel_error(kind: KernelErrorKind) -> KernelError {
        KernelError {
            operation: Operation::Fillet,
            kind,
            exception: "StdFail_NotDone".to_string(),
            message: String::new(),
        }
    }

    #[test]
    fn a_selected_edge_keeps_its_faces() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let shape = state.shape().expect("a solid").clone();
        let front = face_index(state.naming(), "f-box1/front");
        let top = face_index(state.naming(), "f-box1/top");
        let fixture = Fixture::new("f-fil1").with_body("b-1", state.clone());
        let edges = [reference(
            r#"{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=5,0,10|l=10"}"#,
        )];

        let selection =
            select_edges(&fixture.context(), &id("b-1"), &state, &shape, &edges).expect("resolved");

        let mut faces = vec![front, top];
        faces.sort_unstable();
        assert_eq!(
            selection,
            EdgeSelection {
                edges: vec![SelectedEdge {
                    index: edge_between(&shape, front, top),
                    faces,
                    source: 0,
                    entries: vec![0],
                }],
                warnings: Vec::new(),
            }
        );
    }

    #[test]
    fn every_edge_of_a_box_has_a_corner() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let topology = topology_info(&cube.shape).expect("topology");

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
        let topology = topology_info(&shape).expect("topology");

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
        let topology = topology_info(&shape).expect("topology");

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
        let seams: Vec<bool> = cylinder
            .topology()
            .expect("kernel topology")
            .edges
            .iter()
            .map(|edge| edge.seam)
            .collect();
        let topology = topology_info(&cylinder).expect("topology");

        for (edge, seam) in (1_u32..).zip(seams) {
            assert_eq!(
                is_smooth(&cylinder, &topology, edge).expect("checked"),
                seam,
                "edge {edge}"
            );
        }
    }

    #[test]
    fn edges_next_to_a_torus_are_smooth() {
        let (shape, faces_of) = filleted_rim();
        let kernel_topology = shape.topology().expect("kernel topology");
        let topology = topology_info(&shape).expect("topology");
        let [side] = faces_of(GeometryKind::Cylinder)[..] else {
            panic!("one cylinder face");
        };
        let [torus] = faces_of(GeometryKind::Torus)[..] else {
            panic!("one torus face");
        };

        let tangent: Vec<u32> = (1_u32..)
            .zip(&kernel_topology.edges)
            .filter(|(_, edge)| edge.faces.contains(&torus) && !edge.seam)
            .map(|(index, _)| index)
            .collect();

        assert_eq!(tangent.len(), 2, "the torus meets the side and the top");
        for edge in tangent {
            assert!(
                is_smooth(&shape, &topology, edge).expect("checked"),
                "edge {edge}"
            );
        }
        let bottom_rim = (1_u32..)
            .zip(&kernel_topology.edges)
            .find(|(_, edge)| {
                edge.faces.contains(&side) && !edge.faces.contains(&torus) && !edge.seam
            })
            .map(|(index, _)| index)
            .expect("the bottom rim");
        assert!(!is_smooth(&shape, &topology, bottom_rim).expect("checked"));
    }

    #[test]
    fn a_knife_edge_between_a_flat_face_and_a_cylinder_has_a_corner() {
        let (shape, knife) = knife_edge_prism();
        let topology = topology_info(&shape).expect("topology");

        let normals = shape.edge_normals(knife).expect("described");

        let [first, second] = normals.faces[..] else {
            panic!("two faces: {normals:?}");
        };
        assert!(
            angle_between(first.normal, second.normal) > PI - 1e-9,
            "the outward normals are opposite: {normals:?}"
        );
        assert!(!is_smooth(&shape, &topology, knife).expect("checked"));
    }

    #[test]
    fn normals_within_the_smooth_angle_meet_without_a_corner() {
        let up = Vector::new(0.0, 0.0, 1.0);

        assert!(meets_without_corner(1, Ok(two_faces(up, up))).expect("decided"));
        assert!(meets_without_corner(1, Ok(two_faces(up, tilted(0.9e-3)))).expect("decided"));
        assert!(!meets_without_corner(1, Ok(two_faces(up, tilted(1.1e-3)))).expect("decided"));
    }

    #[test]
    fn a_recorded_tangency_meets_without_a_corner_whatever_the_normals() {
        let mut normals = two_faces(Vector::new(0.0, 0.0, 1.0), tilted(0.1));
        normals.continuity = Some(Continuity::G1OrBetter);

        assert!(meets_without_corner(1, Ok(normals)).expect("decided"));
    }

    #[test]
    fn a_recorded_corner_or_no_record_leaves_the_decision_to_the_normals() {
        let up = Vector::new(0.0, 0.0, 1.0);
        let mut normals = two_faces(up, tilted(0.1));
        normals.continuity = None;

        assert!(!meets_without_corner(1, Ok(two_faces(up, tilted(0.1)))).expect("decided"));
        assert!(!meets_without_corner(1, Ok(normals)).expect("decided"));
    }

    #[test]
    fn opposite_normals_are_a_knife_edge_with_a_corner() {
        let up = Vector::new(0.0, 0.0, 1.0);
        let down = Vector::new(0.0, 0.0, -1.0);

        assert!(!meets_without_corner(1, Ok(two_faces(up, down))).expect("decided"));
    }

    #[test]
    fn an_edge_the_kernel_rejects_as_invalid_input_has_a_corner() {
        let rejected = meets_without_corner(1, Err(kernel_error(KernelErrorKind::InvalidInput)));

        assert_eq!(rejected, Ok(false));
    }

    #[test]
    fn other_kernel_failures_of_the_edge_query_are_reported() {
        let error = meets_without_corner(1, Err(kernel_error(KernelErrorKind::Failed)))
            .expect_err("the query failed");

        assert_eq!(
            error,
            FeatureError::from(kernel_error(KernelErrorKind::Failed))
        );
    }

    #[test]
    fn edge_normals_with_other_than_two_faces_are_internal() {
        let mut normals = two_faces(Vector::new(0.0, 0.0, 1.0), Vector::new(0.0, 0.0, 1.0));
        normals.faces.pop();

        let error = meets_without_corner(7, Ok(normals)).expect_err("one face");

        assert_eq!(error.code(), "internal");
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
            |_, _| Ok(FeatureError::internal("not expected")),
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
                Ok(FeatureError::from(cause))
            },
        )
        .expect_err("the requested value fails");

        assert_eq!(
            error,
            FeatureError::from(kernel_error(KernelErrorKind::Failed))
        );
        let max = largest.get().expect("some value works");
        assert!(max <= 2.0 && max > 1.99, "{max}");
    }

    #[test]
    fn a_working_value_is_returned() {
        let fixture = Fixture::new("f-fil1");

        let value = run_or_search(&fixture.context(), 3.0, Ok, |_, _| {
            Ok(FeatureError::internal("not expected"))
        })
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
            .expect("the result is named")
            .naming;

        assert_eq!(
            face_names(&naming),
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
            .expect("the result is named")
            .naming;

        let names = face_names(&naming);
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
        let fixture = Fixture::new("f-fil1");

        let output = no_effect(&fixture.context(), &id("b-1"), &state, 2, Vec::new());

        // Two entries that were all skipped (features.md section 3.15).
        assert_eq!(
            output.warnings,
            [FeatureWarning::NoEffect {
                body: id("b-1"),
                how: NoEffectHow::Skipped {
                    field: fixture.context().field("edges"),
                },
                reverse_hits: false,
                attach: None,
            }]
        );
        let crate::OutputData::Bodies(updates) = output.data else {
            panic!("a body update");
        };
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].change, BodyChange::Modify);
        assert!(Arc::ptr_eq(&updates[0].naming, state.naming()));
    }
}
