//! Fixtures and helpers shared by the acceptance tests (spec section 6).

// Each test binary uses only some of these fixtures.
#![allow(dead_code)]

use std::sync::Arc;

use partsmith_model::Id;
use partsmith_naming::{
    Aliases, Anchors, Candidate, EdgeInfo, ElementInfo, ElementKind, ElementName, ElementSet,
    FaceInfo, FaceSource, Fingerprint, Follow, GeometryKind, Lost, MatchMethod, Merged, MergedName,
    Motion, NamingMap, Reference, Resolution, Successions, TopologyInfo, VertexInfo,
};
use proptest::prelude::*;
use serde_json::Value;

/// Tolerance for scores and confidences, which the spec gives rounded to 6 decimals.
pub const SCORE_TOLERANCE: f64 = 1e-6;

/// Tolerance for candidate distances, which the spec gives rounded to 6 decimals.
pub const DISTANCE_TOLERANCE: f64 = 1e-6;

/// Tolerance for the components of composed motions (spec test 139).
pub const MOTION_TOLERANCE: f64 = 1e-12;

/// Fingerprint of the BOX top face with adjacency counts (spec section 3.7.3).
pub const TOP_FINGERPRINT: &str = "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4";

/// Fingerprint of BOX edge E2, the top front edge, with adjacency counts (spec test 56).
pub const TOP_FRONT_EDGE_FINGERPRINT: &str = "line|n=1,0,0|c=5,0,30|l=10|adj=2,2";

/// The fingerprint of a face whose fingerprint a resolver test does not state (spec section 6).
pub const DEFAULT_FACE_FINGERPRINT: &str = "plane|n=0,0,1|c=0,0,0";

/// The fingerprint of an edge whose fingerprint a resolver test does not state (spec section 6).
pub const DEFAULT_EDGE_FINGERPRINT: &str = "line|n=1,0,0|c=0,0,0";

/// The fingerprint of a vertex whose fingerprint a resolver test does not state (spec section 6).
pub const DEFAULT_VERTEX_FINGERPRINT: &str = "point|c=0,0,0";

/// BOX sources: the names of faces 1 to 6.
pub const BOX_FACE_NAMES: [&str; 6] = [
    "f-box1/left",
    "f-box1/right",
    "f-box1/front",
    "f-box1/back",
    "f-box1/bottom",
    "f-box1/top",
];

/// Names of BOX edges E1 to E12 (spec test 33).
pub const BOX_EDGE_NAMES: [&str; 12] = [
    "edge(f-box1/bottom|f-box1/front)",
    "edge(f-box1/front|f-box1/top)",
    "edge(f-box1/back|f-box1/bottom)",
    "edge(f-box1/back|f-box1/top)",
    "edge(f-box1/bottom|f-box1/left)",
    "edge(f-box1/left|f-box1/top)",
    "edge(f-box1/bottom|f-box1/right)",
    "edge(f-box1/right|f-box1/top)",
    "edge(f-box1/front|f-box1/left)",
    "edge(f-box1/back|f-box1/left)",
    "edge(f-box1/front|f-box1/right)",
    "edge(f-box1/back|f-box1/right)",
];

/// Names of BOX vertices V1 to V8 (spec test 33).
pub const BOX_VERTEX_NAMES: [&str; 8] = [
    "vertex(f-box1/bottom|f-box1/front|f-box1/left)",
    "vertex(f-box1/front|f-box1/left|f-box1/top)",
    "vertex(f-box1/back|f-box1/bottom|f-box1/left)",
    "vertex(f-box1/back|f-box1/left|f-box1/top)",
    "vertex(f-box1/bottom|f-box1/front|f-box1/right)",
    "vertex(f-box1/front|f-box1/right|f-box1/top)",
    "vertex(f-box1/back|f-box1/bottom|f-box1/right)",
    "vertex(f-box1/back|f-box1/right|f-box1/top)",
];

/// Every geometry kind, in declaration order.
pub const ALL_GEOMETRY_KINDS: [GeometryKind; 21] = [
    GeometryKind::Plane,
    GeometryKind::Cylinder,
    GeometryKind::Cone,
    GeometryKind::Sphere,
    GeometryKind::Torus,
    GeometryKind::Revolution,
    GeometryKind::Extrusion,
    GeometryKind::BsplineSurface,
    GeometryKind::BezierSurface,
    GeometryKind::OffsetSurface,
    GeometryKind::OtherSurface,
    GeometryKind::Line,
    GeometryKind::Circle,
    GeometryKind::Ellipse,
    GeometryKind::Hyperbola,
    GeometryKind::Parabola,
    GeometryKind::BsplineCurve,
    GeometryKind::BezierCurve,
    GeometryKind::OffsetCurve,
    GeometryKind::OtherCurve,
    GeometryKind::Point,
];

/// Parses an element name the test knows to be valid.
pub fn name(text: &str) -> ElementName {
    ElementName::parse(text)
        .unwrap_or_else(|error| panic!("{text:?} is a valid element name, got {error:?}"))
}

/// Parses element names the test knows to be valid, in the given order.
pub fn names(texts: &[&str]) -> Vec<ElementName> {
    texts.iter().map(|text| name(text)).collect()
}

/// Builds an id the test knows to be valid.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|error| panic!("{text:?} is a valid id, got {error:?}"))
}

/// Parses a fingerprint text the test knows to be valid.
pub fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint::parse(text)
        .unwrap_or_else(|error| panic!("{text:?} is a valid fingerprint, got {error:?}"))
}

/// The fingerprint a resolver test uses for an element of `kind` whose fingerprint it does not
/// state (spec section 6).
pub fn default_fingerprint(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::Face => DEFAULT_FACE_FINGERPRINT,
        ElementKind::Edge => DEFAULT_EDGE_FINGERPRINT,
        ElementKind::Vertex => DEFAULT_VERTEX_FINGERPRINT,
    }
}

/// Builds a reference whose query and fingerprint the test knows to be valid and of one kind.
pub fn reference(query: &str, fingerprint_text: &str) -> Reference {
    Reference::new(name(query), fingerprint(fingerprint_text)).unwrap_or_else(|error| {
        panic!("{query:?} with {fingerprint_text:?} is a valid reference, got {error:?}")
    })
}

/// A reference to `query` with the default fingerprint of its kind (spec section 6).
pub fn query(text: &str) -> Reference {
    reference(text, default_fingerprint(name(text).kind()))
}

/// The same reference with these anchors: `(part, anchor names)` pairs.
pub fn with_anchors(mut reference: Reference, entries: &[(&str, &[&str])]) -> Reference {
    reference.anchors = anchors(entries);
    reference
}

/// The same reference with these merged names: `(face name, merged name texts)` pairs.
pub fn with_merged(mut reference: Reference, entries: &[(&str, &[&str])]) -> Reference {
    reference.merged = merged(entries);
    reference
}

/// Anchors from `(part, anchor names)` pairs, each list as written.
pub fn anchors(entries: &[(&str, &[&str])]) -> Anchors {
    Anchors(
        entries
            .iter()
            .map(|(part, anchor_names)| (name(part), names(anchor_names)))
            .collect(),
    )
}

/// Parses the text form of a merged name (`f-box2/top` or `f-box1/top@f-lp1:*`).
pub fn merged_name(text: &str) -> MergedName {
    serde_json::from_value(Value::String(text.to_owned()))
        .unwrap_or_else(|error| panic!("{text:?} is a valid merged name, got {error}"))
}

/// Merged names from `(face name, merged name texts)` pairs, each list as written.
pub fn merged(entries: &[(&str, &[&str])]) -> Merged {
    Merged(
        entries
            .iter()
            .map(|(face, texts)| {
                (
                    name(face),
                    texts.iter().map(|text| merged_name(text)).collect(),
                )
            })
            .collect(),
    )
}

/// Merge aliases from `(former name, target names)` pairs, each target list as written.
pub fn aliases(entries: &[(&str, &[&str])]) -> Aliases {
    Aliases(
        entries
            .iter()
            .map(|(former, targets)| (name(former), names(targets)))
            .collect(),
    )
}

/// Sketch successions from `(replaced face, faces that carry its place now)` pairs, in order.
pub fn successions(entries: &[(&str, &[&str])]) -> Successions {
    Successions(
        entries
            .iter()
            .map(|(replaced, now)| (name(replaced), names(now)))
            .collect(),
    )
}

/// The motion that translates by `(x, y, z)` without turning.
pub fn translation(x: f64, y: f64, z: f64) -> Motion {
    Motion::new(
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        [x, y, z],
    )
}

/// The 12 components of a motion read through its public API: the matrix rows, then the
/// translation. Column `j` of the matrix is the image of the `j`-th unit direction.
pub fn motion_components(motion: &Motion) -> [f64; 12] {
    let columns = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .map(|direction| motion.apply_direction(direction));
    let mut components = [0.0; 12];
    for (column, image) in columns.iter().enumerate() {
        for (row, value) in image.iter().enumerate() {
            components[row * 3 + column] = *value;
        }
    }
    components[9..].copy_from_slice(&motion.apply_point([0.0, 0.0, 0.0]));
    components
}

/// Asserts that every component of `actual` is within `MOTION_TOLERANCE` of `expected`.
pub fn assert_motion_close(actual: &Motion, expected: &Motion) {
    let actual_components = motion_components(actual);
    let expected_components = motion_components(expected);
    for (index, (actual_value, expected_value)) in actual_components
        .iter()
        .zip(expected_components)
        .enumerate()
    {
        assert!(
            (actual_value - expected_value).abs() <= MOTION_TOLERANCE,
            "component {index}: expected {expected_components:?}, got {actual_components:?}"
        );
    }
}

/// `FaceSource::New` of a name.
pub fn new_source(text: &str) -> FaceSource {
    FaceSource::New(name(text))
}

/// `FaceSource::From` of input face names, every one from the first inherited map (spec section
/// 6, "Sources in building tests").
pub fn from_sources(texts: &[&str]) -> FaceSource {
    FaceSource::From(texts.iter().map(|text| (0, name(text))).collect())
}

/// `FaceSource::From` of `(inherited map index, name)` pairs.
pub fn from_indexed_sources(sources: &[(u32, &str)]) -> FaceSource {
    FaceSource::From(
        sources
            .iter()
            .map(|(map, text)| (*map, name(text)))
            .collect(),
    )
}

/// One face with its bounding edges.
pub fn face_info(edges: &[u32], fingerprint_text: &str) -> FaceInfo {
    FaceInfo {
        edges: edges.to_vec(),
        fingerprint: fingerprint(fingerprint_text),
    }
}

/// One non-degenerate edge with its faces and vertices.
pub fn edge_info(faces: &[u32], vertices: &[u32], fingerprint_text: &str) -> EdgeInfo {
    EdgeInfo {
        faces: faces.to_vec(),
        vertices: vertices.to_vec(),
        degenerate: false,
        fingerprint: fingerprint(fingerprint_text),
    }
}

/// One degenerated edge (a pole or apex) with its faces and vertices.
pub fn degenerate_edge_info(faces: &[u32], vertices: &[u32], fingerprint_text: &str) -> EdgeInfo {
    EdgeInfo {
        degenerate: true,
        ..edge_info(faces, vertices, fingerprint_text)
    }
}

/// One vertex with its edges and faces.
pub fn vertex_info(edges: &[u32], faces: &[u32], fingerprint_text: &str) -> VertexInfo {
    VertexInfo {
        edges: edges.to_vec(),
        faces: faces.to_vec(),
        fingerprint: fingerprint(fingerprint_text),
    }
}

/// A topology of faces only (no edges, no vertices), one face per fingerprint.
pub fn faces_only_topology(fingerprints: &[&str]) -> TopologyInfo {
    TopologyInfo {
        faces: fingerprints
            .iter()
            .map(|text| face_info(&[], text))
            .collect(),
        edges: Vec::new(),
        vertices: Vec::new(),
    }
}

/// A topology of `count` unit faces in a row along x (face `i` at x = i - 1), without edges.
pub fn face_row_topology(count: usize) -> TopologyInfo {
    TopologyInfo {
        faces: (0..count)
            .map(|position| face_info(&[], &format!("plane|n=0,0,1|c={position},0,0|a=1")))
            .collect(),
        edges: Vec::new(),
        vertices: Vec::new(),
    }
}

/// Builds a map with every option: inherited maps, added successions and the new motion.
pub fn build_with(
    sources: Vec<FaceSource>,
    topology: &TopologyInfo,
    inherited: &[&NamingMap],
    added_successions: &Successions,
    new_motion: &Motion,
) -> NamingMap {
    NamingMap::build(sources, topology, inherited, added_successions, new_motion)
        .unwrap_or_else(|error| panic!("the fixture builds, got {error:?}"))
}

/// Builds a map on inherited maps, without added successions and with the identity as the new
/// motion (spec section 6, "Sources in building tests").
pub fn build_on(
    sources: Vec<FaceSource>,
    topology: &TopologyInfo,
    inherited: &[&NamingMap],
) -> NamingMap {
    build_with(
        sources,
        topology,
        inherited,
        &Successions::default(),
        &Motion::IDENTITY,
    )
}

/// Builds a map without inherited maps, added successions or motion.
pub fn build(sources: Vec<FaceSource>, topology: &TopologyInfo) -> NamingMap {
    build_on(sources, topology, &[])
}

/// A map whose faces are these new names, in a row along x, without edges or vertices: an
/// inherited map for building tests.
pub fn map_with_faces(face_names: &[&str]) -> NamingMap {
    build(
        face_names.iter().map(|text| new_source(text)).collect(),
        &face_row_topology(face_names.len()),
    )
}

/// Deserializes a naming map from JSON the test knows to be valid (spec section 2.6). The only way
/// to give a map aliases or successions without building the features that record them.
pub fn map_from_json(value: Value) -> NamingMap {
    serde_json::from_value(value.clone())
        .unwrap_or_else(|error| panic!("{value} is a valid naming map, got {error}"))
}

/// The names of every element of `kind` in index order, `None` for unnamed elements.
pub fn names_of(map: &NamingMap, kind: ElementKind) -> Vec<Option<&str>> {
    (1..=map.count(kind))
        .map(|index| map.name(kind, index).map(ElementName::as_str))
        .collect()
}

/// BOX topology: a 10 x 20 x 30 box at the origin (spec section 6).
pub fn box_topology() -> TopologyInfo {
    TopologyInfo {
        faces: vec![
            face_info(&[5, 6, 9, 10], "plane|n=-1,0,0|c=0,10,15|a=600"),
            face_info(&[7, 8, 11, 12], "plane|n=1,0,0|c=10,10,15|a=600"),
            face_info(&[1, 2, 9, 11], "plane|n=0,-1,0|c=5,0,15|a=300"),
            face_info(&[3, 4, 10, 12], "plane|n=0,1,0|c=5,20,15|a=300"),
            face_info(&[1, 3, 5, 7], "plane|n=0,0,-1|c=5,10,0|a=200"),
            face_info(&[2, 4, 6, 8], "plane|n=0,0,1|c=5,10,30|a=200"),
        ],
        edges: vec![
            edge_info(&[3, 5], &[1, 5], "line|n=1,0,0|c=5,0,0|l=10"),
            edge_info(&[3, 6], &[2, 6], "line|n=1,0,0|c=5,0,30|l=10"),
            edge_info(&[4, 5], &[3, 7], "line|n=1,0,0|c=5,20,0|l=10"),
            edge_info(&[4, 6], &[4, 8], "line|n=1,0,0|c=5,20,30|l=10"),
            edge_info(&[1, 5], &[1, 3], "line|n=0,1,0|c=0,10,0|l=20"),
            edge_info(&[1, 6], &[2, 4], "line|n=0,1,0|c=0,10,30|l=20"),
            edge_info(&[2, 5], &[5, 7], "line|n=0,1,0|c=10,10,0|l=20"),
            edge_info(&[2, 6], &[6, 8], "line|n=0,1,0|c=10,10,30|l=20"),
            edge_info(&[1, 3], &[1, 2], "line|n=0,0,1|c=0,0,15|l=30"),
            edge_info(&[1, 4], &[3, 4], "line|n=0,0,1|c=0,20,15|l=30"),
            edge_info(&[2, 3], &[5, 6], "line|n=0,0,1|c=10,0,15|l=30"),
            edge_info(&[2, 4], &[7, 8], "line|n=0,0,1|c=10,20,15|l=30"),
        ],
        vertices: vec![
            vertex_info(&[1, 5, 9], &[1, 3, 5], "point|c=0,0,0"),
            vertex_info(&[2, 6, 9], &[1, 3, 6], "point|c=0,0,30"),
            vertex_info(&[3, 5, 10], &[1, 4, 5], "point|c=0,20,0"),
            vertex_info(&[4, 6, 10], &[1, 4, 6], "point|c=0,20,30"),
            vertex_info(&[1, 7, 11], &[2, 3, 5], "point|c=10,0,0"),
            vertex_info(&[2, 8, 11], &[2, 3, 6], "point|c=10,0,30"),
            vertex_info(&[3, 7, 12], &[2, 4, 5], "point|c=10,20,0"),
            vertex_info(&[4, 8, 12], &[2, 4, 6], "point|c=10,20,30"),
        ],
    }
}

/// BOX sources: `New` names for faces 1 to 6.
pub fn box_sources() -> Vec<FaceSource> {
    BOX_FACE_NAMES.iter().map(|text| new_source(text)).collect()
}

/// The map built from BOX sources and BOX topology.
pub fn box_map() -> NamingMap {
    build(box_sources(), &box_topology())
}

/// The element infos of the BOX map.
pub fn box_infos() -> Vec<ElementInfo> {
    box_map()
        .element_infos(&box_topology())
        .unwrap_or_else(|error| panic!("the BOX map matches the BOX topology, got {error:?}"))
}

/// BOX scope: the element set of the BOX infos, without aliases (spec section 6).
pub fn box_scope() -> ElementSet {
    element_set(box_infos())
}

/// The BOX infos without the elements named in `removed`, followed by `added`.
pub fn box_infos_with(removed: &[&str], added: Vec<ElementInfo>) -> Vec<ElementInfo> {
    box_infos()
        .into_iter()
        .filter(|info| !removed.contains(&info.name.as_str()))
        .chain(added)
        .collect()
}

/// CYL topology: a cylinder of radius 5 and height 10 on XY (spec section 6).
pub fn cylinder_topology() -> TopologyInfo {
    TopologyInfo {
        faces: vec![
            face_info(&[1, 2, 3], "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5"),
            face_info(&[1], "plane|n=0,0,1|c=0,0,10|a=78.539816"),
            face_info(&[3], "plane|n=0,0,-1|c=0,0,0|a=78.539816"),
        ],
        edges: vec![
            edge_info(&[1, 2], &[1], "circle|n=0,0,1|c=0,0,10|l=31.415927|r=5"),
            edge_info(&[1], &[1, 2], "line|n=0,0,1|c=5,0,5|l=10"),
            edge_info(&[1, 3], &[2], "circle|n=0,0,1|c=0,0,0|l=31.415927|r=5"),
        ],
        vertices: vec![
            vertex_info(&[1, 2], &[1, 2], "point|c=5,0,10"),
            vertex_info(&[2, 3], &[1, 3], "point|c=5,0,0"),
        ],
    }
}

/// CYL sources: side, top and bottom.
pub fn cylinder_sources() -> Vec<FaceSource> {
    vec![
        new_source("f-cyl1/side"),
        new_source("f-cyl1/top"),
        new_source("f-cyl1/bottom"),
    ]
}

/// The map built from CYL sources and CYL topology.
pub fn cylinder_map() -> NamingMap {
    build(cylinder_sources(), &cylinder_topology())
}

/// SPHERE topology: a sphere of radius 10 (spec section 6).
pub fn sphere_topology() -> TopologyInfo {
    TopologyInfo {
        faces: vec![face_info(&[1, 2, 3], "sphere|c=0,0,0|a=1256.637061|r=10")],
        edges: vec![
            degenerate_edge_info(&[1], &[1], "other_curve|c=0,0,10|l=0"),
            edge_info(
                &[1],
                &[1, 2],
                "circle|n=0,1,0|c=6.366198,0,0|l=31.415927|r=10",
            ),
            degenerate_edge_info(&[1], &[2], "other_curve|c=0,0,-10|l=0"),
        ],
        vertices: vec![
            vertex_info(&[1, 2], &[1], "point|c=0,0,10"),
            vertex_info(&[2, 3], &[1], "point|c=0,0,-10"),
        ],
    }
}

/// The map built from the SPHERE source and SPHERE topology.
pub fn sphere_map() -> NamingMap {
    build(vec![new_source("f-sph1/surface")], &sphere_topology())
}

/// One element info with every field given, without vertices.
pub fn element_info(
    name_text: &str,
    kind: ElementKind,
    fingerprint_text: &str,
    adjacency: &[&str],
) -> ElementInfo {
    ElementInfo {
        name: name(name_text),
        kind,
        fingerprint: fingerprint(fingerprint_text),
        adjacency: names(adjacency),
        vertices: Vec::new(),
    }
}

/// A scope element named `text` as resolver tests list it when they state nothing else (spec
/// section 6): the default fingerprint of its kind, no vertices, and as adjacency the face terms
/// of its name for an edge or vertex, none for a face.
pub fn scope_element(text: &str) -> ElementInfo {
    let element_name = name(text);
    let kind = element_name.kind();
    ElementInfo {
        adjacency: element_name.face_terms(),
        name: element_name,
        kind,
        fingerprint: fingerprint(default_fingerprint(kind)),
        vertices: Vec::new(),
    }
}

/// Changes to a scope element, for the fields a resolver test states.
pub trait ScopeElement {
    /// The same element with this fingerprint.
    fn with_fingerprint(self, fingerprint_text: &str) -> Self;
    /// The same element with this adjacency, as written.
    fn with_adjacency(self, adjacency: &[&str]) -> Self;
    /// The same edge with these named vertices, as written.
    fn with_vertices(self, vertices: &[&str]) -> Self;
}

impl ScopeElement for ElementInfo {
    fn with_fingerprint(self, fingerprint_text: &str) -> Self {
        ElementInfo {
            fingerprint: fingerprint(fingerprint_text),
            ..self
        }
    }

    fn with_adjacency(self, adjacency: &[&str]) -> Self {
        ElementInfo {
            adjacency: names(adjacency),
            ..self
        }
    }

    fn with_vertices(self, vertices: &[&str]) -> Self {
        ElementInfo {
            vertices: names(vertices),
            ..self
        }
    }
}

/// Builds an element set the test knows to be valid.
pub fn element_set_with(
    infos: Vec<ElementInfo>,
    aliases: Aliases,
    successions: Successions,
) -> ElementSet {
    ElementSet::new(Arc::from(infos), aliases, successions)
        .unwrap_or_else(|error| panic!("the scope has no duplicate names, got {error:?}"))
}

/// An element set without aliases and successions.
pub fn element_set(infos: Vec<ElementInfo>) -> ElementSet {
    element_set_with(infos, Aliases::default(), Successions::default())
}

/// The element set of a built map: its element infos, aliases and successions.
pub fn element_set_of_map(map: &NamingMap, topology: &TopologyInfo) -> ElementSet {
    let infos = map
        .element_infos(topology)
        .unwrap_or_else(|error| panic!("the map matches its topology, got {error:?}"));
    element_set_with(infos, map.aliases().clone(), map.successions().clone())
}

/// The info named `text` in `infos`.
pub fn info_named<'a>(infos: &'a [ElementInfo], text: &str) -> &'a ElementInfo {
    infos
        .iter()
        .find(|info| info.name.as_str() == text)
        .unwrap_or_else(|| panic!("the scope contains {text:?}"))
}

/// The info named `text` in `set`.
pub fn element_named<'a>(set: &'a ElementSet, text: &str) -> &'a ElementInfo {
    info_named(set.elements(), text)
}

/// Asserts that `actual` is within `tolerance` of `expected`.
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} within {tolerance}, got {actual}"
    );
}

/// The fields of a `Resolved` outcome, names as text.
pub struct Resolved<'a> {
    /// The elements, as text.
    pub elements: Vec<&'a str>,
    /// The resolver step.
    pub method: MatchMethod,
    /// The confidence.
    pub confidence: f64,
    /// How the reference followed a change.
    pub follow: Option<Follow>,
    /// The keys the match went through, as text.
    pub via: Vec<&'a str>,
    /// The alternatives of a guess.
    pub alternatives: &'a [Candidate],
}

/// The fields of a `Resolved` outcome; panics on any other outcome.
pub fn resolved(resolution: &Resolution) -> Resolved<'_> {
    let Resolution::Resolved {
        elements,
        method,
        confidence,
        follow,
        via,
        alternatives,
    } = resolution
    else {
        panic!("expected a resolved reference, got {resolution:?}");
    };
    Resolved {
        elements: elements.iter().map(ElementName::as_str).collect(),
        method: *method,
        confidence: *confidence,
        follow: *follow,
        via: via.iter().map(ElementName::as_str).collect(),
        alternatives,
    }
}

/// Asserts a `Resolved` outcome with these elements, method and confidence (within 1e-6), and no
/// alternatives unless it is a guess.
pub fn assert_resolved(
    resolution: &Resolution,
    expected_elements: &[&str],
    expected_method: MatchMethod,
    expected_confidence: f64,
) {
    let outcome = resolved(resolution);
    assert_eq!(outcome.elements, expected_elements, "{resolution:?}");
    assert_eq!(outcome.method, expected_method, "{resolution:?}");
    assert_close(outcome.confidence, expected_confidence, SCORE_TOLERANCE);
    if expected_method != MatchMethod::Guess {
        assert!(
            outcome.alternatives.is_empty(),
            "only a guess has alternatives, got {resolution:?}"
        );
    }
}

/// Asserts a `Guess` with these elements, confidence and alternatives in this order.
pub fn assert_guess(
    resolution: &Resolution,
    expected_elements: &[&str],
    expected_confidence: f64,
    expected_alternatives: &[&str],
) {
    assert_resolved(
        resolution,
        expected_elements,
        MatchMethod::Guess,
        expected_confidence,
    );
    assert_eq!(
        candidate_names(resolved(resolution).alternatives),
        expected_alternatives,
        "{resolution:?}"
    );
}

/// Asserts the `follow` of a `Resolved` outcome.
pub fn assert_follow(resolution: &Resolution, expected: Option<Follow>) {
    assert_eq!(resolved(resolution).follow, expected, "{resolution:?}");
}

/// Asserts the `via` of a `Resolved` outcome.
pub fn assert_via(resolution: &Resolution, expected: &[&str]) {
    assert_eq!(resolved(resolution).via, expected, "{resolution:?}");
}

/// The `lost` and `candidates` of a `Missing` outcome; panics on any other outcome.
pub fn missing(resolution: &Resolution) -> (&Lost, &[Candidate]) {
    let Resolution::Missing { lost, candidates } = resolution else {
        panic!("expected a missing reference, got {resolution:?}");
    };
    (lost, candidates)
}

/// The candidates of an `Ambiguous` outcome; panics on any other outcome.
pub fn ambiguous(resolution: &Resolution) -> &[Candidate] {
    let Resolution::Ambiguous { candidates } = resolution else {
        panic!("expected an ambiguous reference, got {resolution:?}");
    };
    candidates
}

/// Asserts what the resolver found gone.
pub fn assert_lost(lost: &Lost, parts: &[&str], pieces: &[&str], unmet: bool) {
    let lost_parts: Vec<&str> = lost.parts.iter().map(ElementName::as_str).collect();
    let lost_pieces: Vec<&str> = lost.pieces.iter().map(ElementName::as_str).collect();
    assert_eq!(lost_parts, parts, "{lost:?}");
    assert_eq!(lost_pieces, pieces, "{lost:?}");
    assert_eq!(lost.unmet, unmet, "{lost:?}");
}

/// Asserts every field of a candidate (distance and score within 1e-6).
pub fn assert_candidate(
    candidate: &Candidate,
    expected_name: &str,
    expected_meaning: bool,
    expected_distance: f64,
    expected_score: f64,
) {
    assert_eq!(candidate.name.as_str(), expected_name, "{candidate:?}");
    assert_eq!(candidate.meaning, expected_meaning, "{candidate:?}");
    assert_close(candidate.distance, expected_distance, DISTANCE_TOLERANCE);
    assert_close(candidate.score, expected_score, SCORE_TOLERANCE);
}

/// Asserts the name, meaning and distance of a candidate (distance within 1e-6).
pub fn assert_candidate_place(
    candidate: &Candidate,
    expected_name: &str,
    expected_meaning: bool,
    expected_distance: f64,
) {
    assert_eq!(candidate.name.as_str(), expected_name, "{candidate:?}");
    assert_eq!(candidate.meaning, expected_meaning, "{candidate:?}");
    assert_close(candidate.distance, expected_distance, DISTANCE_TOLERANCE);
}

/// The names of candidates, in order.
pub fn candidate_names(candidates: &[Candidate]) -> Vec<&str> {
    candidates
        .iter()
        .map(|candidate| candidate.name.as_str())
        .collect()
}

/// Test 92's scope: three pieces of a top face, the second split again, each next to other walls.
pub fn renumbered_pieces_infos() -> Vec<ElementInfo> {
    vec![
        scope_element("f-b/top#1").with_adjacency(&["f-b/back", "f-b/front", "f-b/left", "f-n/w"]),
        scope_element("f-b/top#2#1").with_adjacency(&["f-b/back", "f-b/front", "f-c/w1", "f-n/w2"]),
        scope_element("f-b/top#2#2").with_adjacency(&[
            "f-b/back",
            "f-b/front",
            "f-b/right",
            "f-c/w2",
        ]),
    ]
}

/// Test 104's merge aliases: the box faces a chamfer was named after merged into other faces.
pub fn nested_aliases() -> Aliases {
    aliases(&[
        ("f-box1/front", &["f-a1/front"]),
        ("f-box1/top", &["f-a1/top"]),
    ])
}

/// Test 104's scope: one chamfer face named after the faces the box faces merged into.
pub fn nested_alias_scope() -> ElementSet {
    element_set_with(
        vec![scope_element("f-cha1/chamfer/edge(f-a1/front|f-a1/top)")],
        nested_aliases(),
        Successions::default(),
    )
}

/// Test 105's merged names: the faces of the query's chamfer name were merged from the box faces.
pub fn nested_merged() -> Merged {
    merged(&[
        ("f-a1/front", &["f-box1/front"]),
        ("f-a1/top", &["f-box1/top"]),
    ])
}

/// Test 105's scope: one chamfer face named after the box faces.
pub fn nested_merged_scope() -> ElementSet {
    element_set(vec![scope_element(
        "f-cha1/chamfer/edge(f-box1/front|f-box1/top)",
    )])
}

/// Raw values for random fingerprints (spec test 27), every magnitude below 1e9.
#[derive(Debug, Clone)]
pub struct FingerprintValues {
    /// A direction long enough to normalize, with components that may be far below the 6-decimal
    /// text precision after normalization.
    pub direction: [f64; 3],
    /// Centroid or position in millimeters.
    pub centroid: [f64; 3],
    /// Area or length.
    pub size: f64,
    /// Radius.
    pub radius: f64,
    /// Adjacency counts, if any.
    pub adjacency: Option<[u32; 2]>,
}

impl FingerprintValues {
    /// The fingerprint of `kind` built from these values with `Fingerprint::new`, which drops the
    /// fields the kind does not allow.
    pub fn fingerprint(&self, kind: GeometryKind) -> Fingerprint {
        let built = Fingerprint::new(
            kind,
            Some(self.direction),
            self.centroid,
            Some(self.size),
            Some(self.radius),
        )
        .unwrap_or_else(|error| panic!("{self:?} are valid {kind:?} values, got {error:?}"));
        match self.adjacency {
            Some(adjacency) => built.with_adjacency(adjacency),
            None => built,
        }
    }
}

/// A coordinate with magnitude below 1e9, mixing large, small and integral values.
pub fn coordinate() -> impl Strategy<Value = f64> {
    prop_oneof![
        -999_999_999.0..999_999_999.0_f64,
        -1000.0..1000.0_f64,
        (-1000_i32..=1000).prop_map(f64::from),
    ]
}

/// A size or radius in [0, 1e9), mixing large, small and integral values.
pub fn magnitude() -> impl Strategy<Value = f64> {
    prop_oneof![
        0.0..999_999_999.0_f64,
        0.0..1000.0_f64,
        (0_i32..=1000).prop_map(f64::from),
    ]
}

/// A direction component: unit or integral (axis aligned and diagonal directions), any real, or a
/// tiny real that normalizes to a component near or below the text precision.
fn direction_component() -> impl Strategy<Value = f64> {
    prop_oneof![
        (-1_i32..=1).prop_map(f64::from),
        (-1000_i32..=1000).prop_map(f64::from),
        -1000.0..1000.0_f64,
        -1e-5..1e-5_f64,
    ]
}

/// A direction long enough for `Fingerprint::new` to normalize (at least 1e-12).
pub fn direction() -> impl Strategy<Value = [f64; 3]> {
    prop::array::uniform3(direction_component()).prop_filter(
        "a direction must be long enough to normalize",
        |components| {
            components
                .iter()
                .map(|component| component * component)
                .sum::<f64>()
                >= 1e-24
        },
    )
}

/// Random values for fingerprints of every kind.
pub fn fingerprint_values() -> impl Strategy<Value = FingerprintValues> {
    (
        direction(),
        prop::array::uniform3(coordinate()),
        magnitude(),
        magnitude(),
        prop::option::of(prop::array::uniform2(any::<u32>())),
    )
        .prop_map(
            |(direction, centroid, size, radius, adjacency)| FingerprintValues {
                direction,
                centroid,
                size,
                radius,
                adjacency,
            },
        )
}
