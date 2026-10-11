//! Fixtures and helpers shared by the acceptance tests (spec section 6).

// Each test binary uses only some of these fixtures.
#![allow(dead_code)]

use partsmith_model::Id;
use partsmith_naming::{
    Aliases, EdgeInfo, ElementInfo, ElementKind, ElementName, FaceInfo, FaceSource, Fingerprint,
    GeometryKind, NamingMap, Reference, TopologyInfo, VertexInfo,
};
use proptest::prelude::*;

/// Tolerance for scores and confidences, which the spec gives rounded to 6 decimals.
pub const SCORE_TOLERANCE: f64 = 1e-6;

/// Fingerprint of the BOX top face with adjacency counts (spec section 3.7.3).
pub const TOP_FINGERPRINT: &str = "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4";

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

/// Builds an id the test knows to be valid.
pub fn id(text: &str) -> Id {
    Id::new(text).unwrap_or_else(|error| panic!("{text:?} is a valid id, got {error:?}"))
}

/// Parses a fingerprint text the test knows to be valid.
pub fn fingerprint(text: &str) -> Fingerprint {
    Fingerprint::parse(text)
        .unwrap_or_else(|error| panic!("{text:?} is a valid fingerprint, got {error:?}"))
}

/// Builds a reference whose query and fingerprint the test knows to be valid and of one kind.
pub fn reference(query: &str, fingerprint_text: &str) -> Reference {
    Reference::new(name(query), fingerprint(fingerprint_text)).unwrap_or_else(|error| {
        panic!("{query:?} with {fingerprint_text:?} is a valid reference, got {error:?}")
    })
}

/// Builds an alias set from `(former name, current split base)` pairs.
pub fn aliases(pairs: &[(&str, &str)]) -> Aliases {
    Aliases(
        pairs
            .iter()
            .map(|(former, current)| (name(former), name(current)))
            .collect(),
    )
}

/// `FaceSource::New` of a name.
pub fn new_source(text: &str) -> FaceSource {
    FaceSource::New(name(text))
}

/// `FaceSource::From` of a list of input face names.
pub fn from_sources(texts: &[&str]) -> FaceSource {
    FaceSource::From(texts.iter().map(|text| name(text)).collect())
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

/// Builds a map without inherited aliases from inputs the test knows to be valid.
pub fn build(sources: Vec<FaceSource>, topology: &TopologyInfo) -> NamingMap {
    build_with_aliases(sources, topology, &[])
}

/// Builds a map with inherited aliases from inputs the test knows to be valid.
pub fn build_with_aliases(
    sources: Vec<FaceSource>,
    topology: &TopologyInfo,
    inherited_aliases: &[&Aliases],
) -> NamingMap {
    NamingMap::build(sources, topology, inherited_aliases)
        .unwrap_or_else(|error| panic!("the fixture builds, got {error:?}"))
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

/// BOX scope: the element infos of the BOX map.
pub fn box_scope() -> Vec<ElementInfo> {
    box_map()
        .element_infos(&box_topology())
        .unwrap_or_else(|error| panic!("the BOX map matches the BOX topology, got {error:?}"))
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

/// One element info for a resolver scope.
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
        adjacency: adjacency.iter().map(|text| name(text)).collect(),
    }
}

/// The info named `text` in `scope`.
pub fn info_named<'a>(scope: &'a [ElementInfo], text: &str) -> &'a ElementInfo {
    scope
        .iter()
        .find(|info| info.name.as_str() == text)
        .unwrap_or_else(|| panic!("the scope contains {text:?}"))
}

/// Asserts that `actual` is within `tolerance` of `expected`.
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} within {tolerance}, got {actual}"
    );
}

/// Raw values for random fingerprints (spec test 27), every magnitude below 1e9.
#[derive(Debug, Clone)]
pub struct FingerprintValues {
    /// A non-zero direction. Its components are integers before normalization, so no normalized
    /// component lies between 0 and the 6-decimal text precision.
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

/// A non-zero direction with integer components, often axis aligned or diagonal.
pub fn direction() -> impl Strategy<Value = [f64; 3]> {
    prop_oneof![
        prop::array::uniform3(-1_i32..=1),
        prop::array::uniform3(-1000_i32..=1000),
    ]
    .prop_filter("a direction must not be zero", |components| {
        *components != [0, 0, 0]
    })
    .prop_map(|components| components.map(f64::from))
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
