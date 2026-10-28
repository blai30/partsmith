//! The cxx bridge to the C++ shim (kernel spec section 3.9), and the decoders of its flat lists.
//!
//! Every function that can fail returns `Result`, so the shim's `trycatch` turns any C++
//! exception into an error. The only functions without `Result` read values cached when a
//! `Shape` is built, or create the empty shape.

#[cxx::bridge(namespace = "partsmith::occt")]
pub(crate) mod bridge {
    /// A point or vector in model space.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Vec3 {
        x: f64,
        y: f64,
        z: f64,
    }

    /// A right-handed frame: an origin, a unit normal (local z) and a unit x axis.
    #[derive(Debug, Clone, Copy)]
    struct Frame {
        origin: Vec3,
        normal: Vec3,
        x_axis: Vec3,
    }

    /// The kind of an indexed sub-shape, in `SubShapeKind` order.
    #[derive(Debug, Clone, Copy)]
    enum ElementKind {
        Solid,
        Shell,
        Face,
        Edge,
        Vertex,
    }

    /// The kind of a whole shape, in `ShapeKind` order.
    #[derive(Debug, Clone, Copy)]
    enum ShapeType {
        Compound,
        CompSolid,
        Solid,
        Shell,
        Face,
        Wire,
        Edge,
        Vertex,
    }

    /// A shape handle, used to pass lists of shapes.
    struct ShapeHandle {
        shape: SharedPtr<Shape>,
    }

    /// One indexed sub-shape.
    #[derive(Debug, Clone, Copy)]
    struct SubShapeData {
        kind: ElementKind,
        index: u32,
    }

    /// The fate of one input element (kernel spec section 3.3).
    struct FateData {
        modified: Vec<u32>,
        generated: Vec<SubShapeData>,
    }

    /// The fates of every face, edge and vertex of one input, in index order.
    struct InputHistoryData {
        faces: Vec<FateData>,
        edges: Vec<FateData>,
        vertices: Vec<FateData>,
    }

    /// A result shape with the history of each input.
    struct OpResultData {
        shape: SharedPtr<Shape>,
        inputs: Vec<InputHistoryData>,
    }

    /// The geometric kind of an element, in `GeometryKind` order.
    #[derive(Debug, Clone, Copy)]
    enum GeometryType {
        Plane,
        Cylinder,
        Cone,
        Sphere,
        Torus,
        Revolution,
        Extrusion,
        BsplineSurface,
        BezierSurface,
        OffsetSurface,
        OtherSurface,
        Line,
        Circle,
        Ellipse,
        Hyperbola,
        Parabola,
        BsplineCurve,
        BezierCurve,
        OffsetCurve,
        OtherCurve,
        Point,
    }

    /// Geometry of one element; the `has_` flags mark the optional values.
    #[derive(Debug, Clone, Copy)]
    struct ElementData {
        kind: GeometryType,
        has_direction: bool,
        direction: Vec3,
        has_location: bool,
        location: Vec3,
        has_radius: bool,
        radius: f64,
        centroid: Vec3,
        measure: f64,
    }

    /// Lists of indices in compressed form: list `i` is `values[offsets[i]..offsets[i + 1]]`.
    struct IndexLists {
        offsets: Vec<u32>,
        values: Vec<u32>,
    }

    /// Adjacency of every face, edge and vertex.
    struct TopologyData {
        face_edges: IndexLists,
        face_vertices: IndexLists,
        edge_faces: IndexLists,
        edge_vertices: IndexLists,
        edge_degenerate: Vec<bool>,
        edge_seam: Vec<bool>,
        vertex_edges: IndexLists,
        vertex_faces: IndexLists,
    }

    /// One face containing an edge, with its outward unit normal at the edge's middle.
    #[derive(Debug, Clone, Copy)]
    struct FaceNormalData {
        face: u32,
        normal: Vec3,
    }

    /// The continuity recorded across an edge, in `Continuity` order, or `Absent` when the edge
    /// is neither between two faces nor a seam.
    #[derive(Debug, Clone, Copy)]
    enum ContinuityType {
        C0,
        G1OrBetter,
        Absent,
    }

    /// The point and unit tangent at the middle of an edge, the normals of its faces there and
    /// the continuity recorded between them.
    struct EdgeNormalsData {
        point: Vec3,
        tangent: Vec3,
        faces: Vec<FaceNormalData>,
        continuity: ContinuityType,
    }

    /// An axis-aligned box.
    #[derive(Debug, Clone, Copy)]
    struct BoundsData {
        min: Vec3,
        max: Vec3,
    }

    /// The kind of a profile curve.
    #[derive(Debug, Clone, Copy)]
    enum CurveType {
        Line,
        Arc,
        Circle,
    }

    /// One oriented profile curve. Curves of one loop are consecutive and in chain order; a
    /// curve runs from its start point to the start point of the next curve of its loop.
    #[derive(Debug, Clone, Copy)]
    struct ProfileCurveData {
        region: u32,
        loop_index: u32,
        kind: CurveType,
        start_x: f64,
        start_y: f64,
        center_x: f64,
        center_y: f64,
        radius: f64,
        clockwise: bool,
    }

    /// Profile faces plus the edge index each input curve produced.
    struct ProfileData {
        shape: SharedPtr<Shape>,
        curve_edges: Vec<u32>,
    }

    /// The role of a swept face.
    #[derive(Debug, Clone, Copy)]
    enum SweepRole {
        Lateral,
        StartCap,
        EndCap,
    }

    /// The role of one swept face; `edge` is the generating profile edge of a lateral face.
    #[derive(Debug, Clone, Copy)]
    struct SweepFaceData {
        role: SweepRole,
        edge: u32,
    }

    /// A swept shape with the role of each face.
    struct SweepData {
        shape: SharedPtr<Shape>,
        faces: Vec<SweepFaceData>,
    }

    /// One edge to fillet.
    #[derive(Debug, Clone, Copy)]
    struct FilletEdgeData {
        edge: u32,
        radius: f64,
    }

    /// One edge to chamfer. With `two_distances`, `distance` is measured on `face` and
    /// `other_distance` on the edge's other face; otherwise both are equal and `face` is unused.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct ChamferEdgeData {
        edge: u32,
        distance: f64,
        other_distance: f64,
        two_distances: bool,
        face: u32,
    }

    /// A face on the split plane and whether its solid lies on the positive side.
    #[derive(Debug, Clone, Copy)]
    struct SectionFaceData {
        face: u32,
        positive: bool,
    }

    /// The pieces of a split with their sides and section faces.
    struct SplitData {
        result: OpResultData,
        solid_positive: Vec<bool>,
        section_faces: Vec<SectionFaceData>,
    }

    /// A tessellation in flat arrays (three values per point or normal).
    struct MeshData {
        positions: Vec<f64>,
        normals: Vec<f64>,
        triangles: Vec<u32>,
        face_ids: Vec<u32>,
        edge_ids: Vec<u32>,
        edge_offsets: Vec<u32>,
        edge_points: Vec<f64>,
        marker_ids: Vec<u32>,
        marker_positions: Vec<f64>,
    }

    /// One STEP body; the color components are sRGB and only meaningful with `has_color`.
    struct StepBodyData {
        shape: SharedPtr<Shape>,
        name: String,
        has_color: bool,
        red: f64,
        green: f64,
        blue: f64,
    }

    /// The bodies of a STEP import, the counts of what was left out or could not be repaired,
    /// and the sentences that describe them.
    struct StepImportData {
        bodies: Vec<StepBodyData>,
        skipped_shapes: usize,
        invalid_solids: usize,
        warnings: Vec<String>,
    }

    // SAFETY: each declaration matches its definition in cpp/shim.h, which the compiler checks
    // when it builds the generated C++. No shim function throws past `trycatch` or keeps a
    // reference to a Rust slice or string after it returns, returned shapes come from
    // `std::make_shared` (the facade still rejects null), and the C++ `Shape` is immutable, so
    // the calls are safe from any thread (kernel spec sections 3.9 and 3.10).
    unsafe extern "C++" {
        include!("partsmith-occt-sys/cpp/shim.h");

        /// The shim's immutable shape with its cached index maps.
        type Shape;

        fn empty_shape() -> SharedPtr<Shape>;
        fn shape_type(shape: &Shape) -> ShapeType;
        fn shape_count(shape: &Shape, kind: ElementKind) -> u32;
        fn shape_solid(shape: &Shape, index: u32) -> Result<SharedPtr<Shape>>;
        fn shape_locate(shape: &Shape, part: &Shape, kind: ElementKind) -> Result<Vec<u32>>;
        fn shape_is_valid(shape: &Shape) -> Result<bool>;
        fn shape_volume(shape: &Shape) -> Result<f64>;
        fn shape_area(shape: &Shape) -> Result<f64>;
        fn shape_center_of_mass(shape: &Shape) -> Result<Vec3>;
        fn shape_bounding_box(shape: &Shape) -> Result<BoundsData>;
        fn shape_element(shape: &Shape, kind: ElementKind, index: u32) -> Result<ElementData>;
        fn shape_elements(shape: &Shape, kind: ElementKind) -> Result<Vec<ElementData>>;
        fn shape_topology(shape: &Shape) -> Result<TopologyData>;
        fn shape_edge_normals(shape: &Shape, edge: u32) -> Result<EdgeNormalsData>;
        fn shape_width_across(shape: &Shape, face: u32, edge: u32) -> Result<f64>;
        fn shape_tessellate(shape: &Shape, chord_deviation: f64, angle: f64) -> Result<MeshData>;
        /// The points of one element's display geometry, three coordinates each: a face's
        /// triangles, an edge's polyline or a vertex's position.
        fn shape_element_display(
            shape: &Shape,
            kind: ElementKind,
            index: u32,
            chord_deviation: f64,
            angle: f64,
        ) -> Result<Vec<f64>>;
        fn shape_to_brep(shape: &Shape) -> Result<UniquePtr<CxxString>>;
        fn shape_from_brep(bytes: &[u8]) -> Result<SharedPtr<Shape>>;

        fn make_box(
            frame: &Frame,
            length: f64,
            width: f64,
            height: f64,
        ) -> Result<SharedPtr<Shape>>;
        fn make_cylinder(frame: &Frame, radius: f64, height: f64) -> Result<SharedPtr<Shape>>;
        fn make_sphere(frame: &Frame, radius: f64) -> Result<SharedPtr<Shape>>;
        fn make_cone(
            frame: &Frame,
            bottom_radius: f64,
            top_radius: f64,
            height: f64,
        ) -> Result<SharedPtr<Shape>>;
        fn make_torus(
            frame: &Frame,
            major_radius: f64,
            minor_radius: f64,
        ) -> Result<SharedPtr<Shape>>;

        fn make_profile(frame: &Frame, curves: &[ProfileCurveData]) -> Result<ProfileData>;
        fn extrude(profile: &Shape, direction: &Vec3, start: f64, end: f64) -> Result<SweepData>;
        fn revolve(
            profile: &Shape,
            origin: &Vec3,
            direction: &Vec3,
            angle: f64,
            full: bool,
        ) -> Result<SweepData>;

        fn fuse(target: &Shape, tools: &[ShapeHandle]) -> Result<OpResultData>;
        fn cut(target: &Shape, tools: &[ShapeHandle]) -> Result<OpResultData>;
        fn common(target: &Shape, tools: &[ShapeHandle]) -> Result<OpResultData>;

        fn fillet(shape: &Shape, edges: &[FilletEdgeData]) -> Result<OpResultData>;
        fn chamfer(shape: &Shape, edges: &[ChamferEdgeData]) -> Result<OpResultData>;
        fn shell(shape: &Shape, remove_faces: &[u32], thickness: f64) -> Result<OpResultData>;

        fn transform(shape: &Shape, matrix: &[f64]) -> Result<OpResultData>;
        fn split(shape: &Shape, plane: &Frame) -> Result<SplitData>;
        fn section(shape: &Shape, plane: &Frame) -> Result<SharedPtr<Shape>>;
        fn compound(shapes: &[ShapeHandle]) -> Result<OpResultData>;

        fn write_step(bodies: &[StepBodyData]) -> Result<UniquePtr<CxxString>>;
        fn read_step(bytes: &[u8]) -> Result<StepImportData>;
    }
}

/// Groups a flat list from the shim into triples; `None` when its length is not a multiple of 3.
pub(crate) fn triples<T: Copy>(values: &[T]) -> Option<Vec<[T; 3]>> {
    let (triples, rest) = values.as_chunks::<3>();
    rest.is_empty().then(|| triples.to_vec())
}

/// Splits a flat list from the shim into the lists `values[offsets[i]..offsets[i + 1]]`; `None`
/// when a range runs backward or past the end of `values`.
pub(crate) fn split_at_offsets<T: Clone>(offsets: &[u32], values: &[T]) -> Option<Vec<Vec<T>>> {
    offsets
        .windows(2)
        .map(|bounds| {
            values
                .get(bounds[0] as usize..bounds[1] as usize)
                .map(<[T]>::to_vec)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_lists_group_into_triples() {
        assert_eq!(
            triples(&[1, 2, 3, 4, 5, 6]),
            Some(vec![[1, 2, 3], [4, 5, 6]])
        );
        assert_eq!(triples::<u32>(&[]), Some(Vec::new()));
    }

    #[test]
    fn lists_of_other_lengths_have_no_triples() {
        assert_eq!(triples(&[1.0, 2.0, 3.0, 4.0]), None);
    }

    #[test]
    fn backward_offsets_split_nothing() {
        assert_eq!(split_at_offsets(&[2, 1], &[4, 5, 6]), None);
    }
}
