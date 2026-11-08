//! Naming bridge tests for direct-generator names on kernel shapes: features.md section 6.9 test 64
//! (`direct_sources` of a fuse), and naming.md section 6 tests 192 to 195 (the names a boolean,
//! the dress-up features, a split, copies and moves carry). The shapes are named with `name_shape`
//! and `name_by_history`, or built by the built-in registry on a body state whose naming map, read
//! from cache JSON, holds direct-generator names. The last tests check how messages describe an
//! element stored by its direct-generator name (features.md section 3.12).

use crate::common;

use std::sync::Arc;

use common::timeline::picked_in;
use common::{ContextFixture, LabelNames, built_body, id, record};
use partsmith_features::{
    BodyState, FeatureOutput, Named, OutputData, Registry, describe_elements, direct_sources,
    name_by_history, name_shape, topology_info,
};
use partsmith_model::Units;
use partsmith_naming::{
    DirectSource, DirectSources, ElementInfo, ElementKind, ElementName, FaceSource, Fingerprint,
    Instance, Motion, NamingMap, Successions, roles,
};
use partsmith_occt_sys::{
    Interrupt, OpResult, Plane, Point, Primitive, PrimitiveFace, Shape, cut, fuse, make_box,
    make_sphere,
};
use serde_json::{Value, json};

/// The box edge test 193 rounds and bevels.
const FRONT_TOP: &str = "edge(f-box1/front|f-box1/top)";

/// The four bottom edges of the box.
const BOTTOM_EDGES: [&str; 4] = [
    "edge(f-box1/bottom|f-box1/front)",
    "edge(f-box1/back|f-box1/bottom)",
    "edge(f-box1/bottom|f-box1/left)",
    "edge(f-box1/bottom|f-box1/right)",
];

/// The four top edges of the box.
const TOP_EDGES: [&str; 4] = [
    "edge(f-box1/front|f-box1/top)",
    "edge(f-box1/back|f-box1/top)",
    "edge(f-box1/left|f-box1/top)",
    "edge(f-box1/right|f-box1/top)",
];

/// The four vertical edges of the box.
const VERTICAL_EDGES: [&str; 4] = [
    "edge(f-box1/front|f-box1/left)",
    "edge(f-box1/back|f-box1/left)",
    "edge(f-box1/front|f-box1/right)",
    "edge(f-box1/back|f-box1/right)",
];

/// The height of the split plane of test 194, half the box's height.
const SPLIT_HEIGHT: f64 = 15.0;

/// Tolerance for the coordinates read from fingerprints.
const COORDINATE_TOLERANCE: f64 = 1e-6;

/// The instance `(1, 0)` of a mirror or of the first pattern copy.
const FIRST_INSTANCE: Instance = Instance {
    first: 1,
    second: 0,
};

/// Parses an element name the test knows to be valid.
fn name(text: &str) -> ElementName {
    ElementName::parse(text).unwrap_or_else(|error| panic!("{text:?} is a valid name: {error}"))
}

/// Every edge of the box.
fn box_edges() -> Vec<&'static str> {
    BOTTOM_EDGES
        .into_iter()
        .chain(TOP_EDGES)
        .chain(VERTICAL_EDGES)
        .collect()
}

/// The direct-generator name test 193 gives the box edge `edge(f-box1/<a>|f-box1/<b>)`:
/// `edge:f-t/<a>-<b>`.
fn tag(edge: &str) -> String {
    let faces = edge
        .strip_prefix("edge(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("{edge} is a box edge name"));
    let face_roles: Vec<&str> = faces
        .split('|')
        .map(|face| {
            face.strip_prefix("f-box1/")
                .unwrap_or_else(|| panic!("{face} is a box face name"))
        })
        .collect();
    format!("edge:f-t/{}", face_roles.join("-"))
}

/// The role word of a kernel primitive face (naming.md section 3.6.4).
fn role_word(face: PrimitiveFace) -> &'static str {
    match face {
        PrimitiveFace::Left => roles::LEFT,
        PrimitiveFace::Right => roles::RIGHT,
        PrimitiveFace::Front => roles::FRONT,
        PrimitiveFace::Back => roles::BACK,
        PrimitiveFace::Bottom => roles::BOTTOM,
        PrimitiveFace::Top => roles::TOP,
        PrimitiveFace::Side => roles::SIDE,
        PrimitiveFace::Surface => roles::SURFACE,
    }
}

/// Names a kernel primitive by the primitive rule for `feature`, with these direct sources.
fn named_primitive(feature: &str, primitive: &Primitive, direct: DirectSources) -> Named {
    let faces = primitive
        .faces
        .iter()
        .map(|face| {
            FaceSource::New(
                ElementName::generated(&id(feature), role_word(*face), None)
                    .expect("a primitive role makes a face name"),
            )
        })
        .collect();
    name_shape(
        &primitive.shape,
        faces,
        direct,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .expect("the primitive is named")
}

/// The same map read back from its cache JSON with `tag_of` of each named edge as its direct
/// list (naming.md section 2.6); an edge that `tag_of` gives nothing holds none.
fn with_edge_tags(map: &NamingMap, tag_of: impl Fn(&str) -> Option<String>) -> NamingMap {
    let mut value = serde_json::to_value(map).expect("a naming map serializes");
    let edges: Vec<Value> = value["edges"]
        .as_array()
        .expect("a naming map lists its edges")
        .iter()
        .map(|edge| match edge.as_str().and_then(&tag_of) {
            Some(direct) => json!([direct]),
            None => Value::Null,
        })
        .collect();
    value["direct"] = json!({ "edges": edges });
    serde_json::from_value(value).expect("the tagged map deserializes")
}

/// The names of every element of `kind` in index order, `None` for unnamed elements.
fn names_of(map: &NamingMap, kind: ElementKind) -> Vec<Option<String>> {
    (1..=map.count(kind))
        .map(|index| map.name(kind, index).map(|found| found.as_str().to_owned()))
        .collect()
}

/// The direct-generator names element `index` of `kind` holds, as text.
fn direct_of(map: &NamingMap, kind: ElementKind, index: u32) -> Vec<&str> {
    map.direct(kind, index)
        .iter()
        .map(ElementName::as_str)
        .collect()
}

/// Every direct-generator name an edge or vertex of `map` holds, with the element's kind and index.
fn held_names(map: &NamingMap) -> Vec<(ElementKind, u32, String)> {
    [ElementKind::Edge, ElementKind::Vertex]
        .into_iter()
        .flat_map(|kind| {
            (1..=map.count(kind)).flat_map(move |index| {
                map.direct(kind, index)
                    .iter()
                    .map(move |held| (kind, index, held.as_str().to_owned()))
            })
        })
        .collect()
}

/// The 1-based index of the vertex of `shape` at height `z`.
fn vertex_at_height(shape: &Shape, z: f64) -> u32 {
    let topology = topology_info(shape).expect("the topology is read");
    let position = topology
        .vertices
        .iter()
        .position(|vertex| {
            vertex
                .fingerprint
                .centroid
                .is_some_and(|centroid| (centroid[2] - z).abs() <= COORDINATE_TOLERANCE)
        })
        .unwrap_or_else(|| panic!("the shape has a vertex at z = {z}"));
    u32::try_from(position + 1).expect("vertex indices fit in u32")
}

/// Test 192's sphere and box, cut: the sphere of radius 10 at the origin, named with or without
/// the poles `vertex:f-sph1/north` (z = 10) and `vertex:f-sph1/south` (z = -10), and the box
/// x -20..20, y -20..20, z -20..-5 named `f-box1`.
struct SphereCut {
    /// The sphere map with the poles' direct-generator names.
    with_poles: NamingMap,
    /// The sphere map without direct sources.
    without_poles: NamingMap,
    /// The box's body state.
    tool: BodyState,
    /// The kernel cut of the sphere by the box.
    result: OpResult,
}

impl SphereCut {
    /// Builds the shapes, names them and cuts the sphere by the box.
    fn new() -> SphereCut {
        let sphere = make_sphere(&Plane::XY, 10.0).expect("the sphere builds");
        let vertex_count = topology_info(&sphere.shape)
            .expect("the topology is read")
            .vertices
            .len();
        let mut poles = vec![DirectSource::None; vertex_count];
        let north = vertex_at_height(&sphere.shape, 10.0);
        let south = vertex_at_height(&sphere.shape, -10.0);
        poles[north as usize - 1] = DirectSource::New(name("vertex:f-sph1/north"));
        poles[south as usize - 1] = DirectSource::New(name("vertex:f-sph1/south"));
        let with_poles = named_primitive(
            "f-sph1",
            &sphere,
            DirectSources {
                edges: Vec::new(),
                vertices: poles,
            },
        )
        .naming;
        let without_poles = named_primitive("f-sph1", &sphere, DirectSources::default()).naming;
        let tool = built_body(json!({
            "id": "f-box1", "type": "box", "body": "b-1", "size": ["40", "40", "15"],
            "position": ["0", "0", "-20"], "op": "new"
        }));
        let result = cut(
            &sphere.shape,
            &[tool.shape().expect("the box is a solid").clone()],
            &Interrupt::new(),
        )
        .expect("the cut succeeds");
        SphereCut {
            with_poles,
            without_poles,
            tool,
            result,
        }
    }

    /// `name_by_history` of the cut for `f-cut1`, over `sphere_map` and the box's map.
    fn named(&self, sphere_map: &NamingMap) -> NamingMap {
        name_by_history(
            &id("f-cut1"),
            &self.result,
            &[sphere_map, self.tool.naming()],
            &Motion::IDENTITY,
        )
        .expect("the cut is named")
        .naming
    }
}

/// Test 193's body state: a 10 x 20 x 30 box `f-box1` named by the box rule, whose naming map,
/// read from cache JSON, gives each edge its tag.
fn tagged_box() -> BodyState {
    let block = built_body(
        json!({"id": "f-box1", "type": "box", "body": "b-1", "size": ["10", "20", "30"], "op": "new"}),
    );
    let naming = with_edge_tags(block.naming(), |edge| Some(tag(edge)));
    BodyState::new(
        block.shape().expect("the box is a solid").clone(),
        Arc::new(naming),
    )
}

/// Builds `value`, a record of the built-in registry, in a millimeter document without parameters,
/// on these body states and upstream outputs, and returns its output.
fn build_on(
    value: Value,
    bodies: &[(&str, &BodyState)],
    upstream: &[(&str, &Arc<FeatureOutput>)],
) -> FeatureOutput {
    let registry = Registry::with_builtins();
    let validated = registry
        .validate_and_canonicalize(&record(value))
        .expect("records written in tests are valid");
    let dependencies = registry
        .dependencies(&validated.record)
        .expect("valid records have dependencies");
    let manifest = registry
        .get(&validated.record.kind)
        .expect("the type is built in")
        .manifest()
        .clone();
    let mut fixture = ContextFixture::with_manifest(
        validated.record.id.clone(),
        manifest,
        Units::Mm,
        &dependencies.expressions,
    );
    for (body, state) in bodies {
        fixture.bodies.insert(id(body), (*state).clone());
    }
    for (feature, output) in upstream {
        fixture.upstream.insert(id(feature), Arc::clone(output));
    }
    registry
        .build(&validated.record, &fixture.context())
        .expect("the record builds")
}

/// The state that `output` gives `body`.
fn state_of(output: &FeatureOutput, body: &str) -> BodyState {
    let OutputData::Bodies(updates) = &output.data else {
        panic!("the feature updates bodies");
    };
    let update = updates
        .iter()
        .find(|update| update.body.as_str() == body)
        .unwrap_or_else(|| panic!("the feature updates {body}"));
    BodyState::from_update(update)
}

/// Builds `value` on test 193's body state in `b-1` (and an empty `b-2`) and returns the state it
/// gives `body`.
fn built_on_tagged_box(value: Value, body: &str) -> BodyState {
    let tagged = tagged_box();
    let empty = BodyState::empty();
    let output = build_on(value, &[("b-1", &tagged), ("b-2", &empty)], &[]);
    state_of(&output, body)
}

/// The edge infos of a body state.
fn edge_infos(state: &BodyState) -> Vec<ElementInfo> {
    state
        .element_set()
        .expect("the body state has an element set")
        .elements()
        .iter()
        .filter(|info| info.kind == ElementKind::Edge)
        .cloned()
        .collect()
}

/// The edges among `edges` that hold `direct`.
fn holders<'a>(edges: &'a [ElementInfo], direct: &str) -> Vec<&'a ElementInfo> {
    edges
        .iter()
        .filter(|info| info.direct.iter().any(|held| held.as_str() == direct))
        .collect()
}

/// The one edge among `edges` that holds `direct`.
fn only_holder<'a>(edges: &'a [ElementInfo], direct: &str) -> &'a ElementInfo {
    let found = holders(edges, direct);
    assert_eq!(
        found.len(),
        1,
        "exactly one edge holds {direct}, got {:?}",
        found
            .iter()
            .map(|info| info.name.as_str())
            .collect::<Vec<_>>()
    );
    found[0]
}

/// The z of an element's fingerprint centroid.
fn height(info: &ElementInfo) -> f64 {
    info.fingerprint
        .centroid
        .unwrap_or_else(|| panic!("{} has a centroid", info.name))[2]
}

/// Asserts that each of `kept` (box edge names) gives exactly one edge of `state`, named after the
/// same faces, that holds its tag, and that no edge holds the tag of any of `gone`.
fn assert_tags_kept(state: &BodyState, kept: &[&str], gone: &[&str]) {
    let edges = edge_infos(state);
    for edge in kept {
        assert_eq!(only_holder(&edges, &tag(edge)).name.as_str(), *edge);
    }
    for edge in gone {
        assert!(
            holders(&edges, &tag(edge)).is_empty(),
            "no edge holds {}",
            tag(edge)
        );
    }
}

/// Asserts that every box edge gives exactly one edge of `state` holding its tag followed by
/// `@<feature>:1`, named after the copies of the same faces.
fn assert_copy_tags(state: &BodyState, feature: &str) {
    let edges = edge_infos(state);
    for edge in box_edges() {
        let copied_name = name(edge)
            .with_instance(&id(feature), FIRST_INSTANCE)
            .expect("a box edge has an instance name");
        let copied_tag = format!("{}@{feature}:1", tag(edge));
        assert_eq!(only_holder(&edges, &copied_tag).name, copied_name);
    }
}

/// The record of a dress-up feature of the front top edge of test 193's box.
fn dress_up_record(feature: &str, kind: &str, size: (&str, Value)) -> Value {
    let picked = picked_in(
        &tagged_box()
            .element_set()
            .expect("the box has an element set"),
        FRONT_TOP,
    );
    let mut value = json!({"id": feature, "type": kind, "body": "b-1", "edges": [picked]});
    value[size.0] = size.1;
    value
}

/// Test 64's shapes: the kernel boxes `A` (10 x 20 x 30 at the origin) and `B` (10 x 10 x 10 at
/// x = 50), named `f-box1` with the front top edge tagged and `f-box2` without tags.
struct DisjointBoxes {
    /// The naming map of `A`.
    first: NamingMap,
    /// The naming map of `B`.
    second: NamingMap,
    /// The kernel fuse of `A` with the tool `B`.
    result: OpResult,
}

impl DisjointBoxes {
    /// Builds and names the boxes and fuses them.
    fn new() -> DisjointBoxes {
        let first_box = make_box(&Plane::XY, 10.0, 20.0, 30.0).expect("box A builds");
        let second_placement = Plane {
            origin: Point::new(50.0, 0.0, 0.0),
            ..Plane::XY
        };
        let second_box = make_box(&second_placement, 10.0, 10.0, 10.0).expect("box B builds");
        let first = with_edge_tags(
            &named_primitive("f-box1", &first_box, DirectSources::default()).naming,
            |edge| (edge == FRONT_TOP).then(|| tag(edge)),
        );
        let second = named_primitive("f-box2", &second_box, DirectSources::default()).naming;
        let result = fuse(
            &first_box.shape,
            std::slice::from_ref(&second_box.shape),
            &Interrupt::new(),
        )
        .expect("the fuse succeeds");
        DisjointBoxes {
            first,
            second,
            result,
        }
    }
}

/// The direct source the kernel history of `result` gives output element `output` of `kind`: the
/// input elements whose fates list it in `modified`.
fn expected_source(result: &OpResult, kind: ElementKind, output: u32) -> DirectSource {
    let mut sources = Vec::new();
    for (input, history) in (0..).zip(&result.history.inputs) {
        let fates = match kind {
            ElementKind::Edge => &history.edges,
            _ => &history.vertices,
        };
        for (element, fate) in (1..).zip(fates) {
            if fate.modified.contains(&output) {
                sources.push((input, element));
            }
        }
    }
    if sources.is_empty() {
        DirectSource::None
    } else {
        DirectSource::From(sources)
    }
}

#[test]
fn t064_direct_sources_of_a_fuse_have_one_entry_per_edge_and_vertex() {
    let boxes = DisjointBoxes::new();

    let direct = direct_sources(&boxes.result, &[&boxes.first, &boxes.second])
        .expect("the fuse has direct sources");

    assert_eq!(direct.edges.len(), 24);
    assert_eq!(direct.vertices.len(), 16);
}

#[test]
fn t064_each_direct_source_of_a_fuse_is_the_one_input_element_it_descends_from() {
    let boxes = DisjointBoxes::new();

    let direct = direct_sources(&boxes.result, &[&boxes.first, &boxes.second])
        .expect("the fuse has direct sources");

    for (kind, sources) in [
        (ElementKind::Edge, &direct.edges),
        (ElementKind::Vertex, &direct.vertices),
    ] {
        for (output, source) in (1..).zip(sources) {
            let DirectSource::From(pairs) = source else {
                panic!("{kind:?} {output} descends from an input element, got {source:?}");
            };
            assert_eq!(pairs.len(), 1, "{kind:?} {output}: {pairs:?}");
            assert_eq!(
                *source,
                expected_source(&boxes.result, kind, output),
                "{kind:?} {output}"
            );
        }
    }
}

#[test]
fn t064_name_by_history_carries_the_tag_of_the_front_top_edge() {
    let boxes = DisjointBoxes::new();
    let front_top = boxes
        .first
        .index(&name(FRONT_TOP))
        .expect("box A has a front top edge");
    let images = &boxes.result.history.inputs[0].edges[front_top as usize - 1].modified;

    let naming = name_by_history(
        &id("f-fuse1"),
        &boxes.result,
        &[&boxes.first, &boxes.second],
        &Motion::IDENTITY,
    )
    .expect("the fuse is named")
    .naming;

    assert_eq!(images.len(), 1, "the front top edge has one image");
    let image = images[0];
    assert_eq!(
        naming.name(ElementKind::Edge, image),
        Some(&name(FRONT_TOP))
    );
    assert_eq!(
        direct_of(&naming, ElementKind::Edge, image),
        ["edge:f-t/front-top"]
    );
    assert_eq!(
        held_names(&naming),
        [(ElementKind::Edge, image, "edge:f-t/front-top".to_owned())]
    );
}

#[test]
fn t064_direct_sources_with_a_missing_input_map_are_internal() {
    let boxes = DisjointBoxes::new();

    let error =
        direct_sources(&boxes.result, &[&boxes.first]).expect_err("the tool's map is missing");

    assert_eq!(error.code(), "internal");
}

#[test]
fn t192_the_sphere_poles_hold_their_direct_generator_names_before_the_cut() {
    let shapes = SphereCut::new();

    let mut held: Vec<(ElementKind, String)> = held_names(&shapes.with_poles)
        .into_iter()
        .map(|(kind, _, held)| (kind, held))
        .collect();
    held.sort();

    assert_eq!(
        held,
        [
            (ElementKind::Vertex, "vertex:f-sph1/north".to_owned()),
            (ElementKind::Vertex, "vertex:f-sph1/south".to_owned()),
        ]
    );
}

#[test]
fn t192_the_north_pole_keeps_its_direct_generator_name_through_the_cut() {
    let shapes = SphereCut::new();

    let naming = shapes.named(&shapes.with_poles);

    let north = vertex_at_height(&shapes.result.shape, 10.0);
    assert_eq!(
        naming.name(ElementKind::Vertex, north),
        Some(&name("vertex(f-sph1/surface)"))
    );
    assert_eq!(
        direct_of(&naming, ElementKind::Vertex, north),
        ["vertex:f-sph1/north"]
    );
}

#[test]
fn t192_no_element_holds_the_south_pole_the_cut_removed() {
    let shapes = SphereCut::new();

    let naming = shapes.named(&shapes.with_poles);

    assert!(
        held_names(&naming)
            .iter()
            .all(|(_, _, held)| held != "vertex:f-sph1/south"),
        "{:?}",
        held_names(&naming)
    );
    assert_eq!(naming.index(&name("vertex:f-sph1/south")), None);
}

#[test]
fn t192_every_name_equals_the_name_without_direct_sources() {
    let shapes = SphereCut::new();

    let with_poles = shapes.named(&shapes.with_poles);
    let without_poles = shapes.named(&shapes.without_poles);

    for kind in [ElementKind::Face, ElementKind::Edge, ElementKind::Vertex] {
        assert_eq!(
            names_of(&with_poles, kind),
            names_of(&without_poles, kind),
            "{kind:?}"
        );
    }
}

#[test]
fn t193_a_fillet_keeps_the_tags_of_the_edges_it_does_not_round() {
    let record = dress_up_record("f-fil1", "fillet", ("radius", json!("1")));

    let state = built_on_tagged_box(record, "b-1");

    let kept: Vec<&str> = box_edges()
        .into_iter()
        .filter(|edge| *edge != FRONT_TOP)
        .collect();
    assert_tags_kept(&state, &kept, &[FRONT_TOP]);
}

#[test]
fn t193_a_chamfer_keeps_the_tags_of_the_edges_it_does_not_bevel() {
    let record = dress_up_record(
        "f-cha1",
        "chamfer",
        ("distance", json!({"kind": "equal", "expr": "1"})),
    );

    let state = built_on_tagged_box(record, "b-1");

    let kept: Vec<&str> = box_edges()
        .into_iter()
        .filter(|edge| *edge != FRONT_TOP)
        .collect();
    assert_tags_kept(&state, &kept, &[FRONT_TOP]);
}

#[test]
fn t193_a_shell_keeps_the_tags_of_the_bottom_and_vertical_edges() {
    let top = picked_in(
        &tagged_box()
            .element_set()
            .expect("the box has an element set"),
        "f-box1/top",
    );
    let record = json!({
        "id": "f-sh1", "type": "shell", "body": "b-1", "thickness": "1", "remove_faces": [top]
    });

    let state = built_on_tagged_box(record, "b-1");

    let kept: Vec<&str> = BOTTOM_EDGES.into_iter().chain(VERTICAL_EDGES).collect();
    assert_tags_kept(&state, &kept, &[]);
}

#[test]
fn t194_a_split_numbers_the_tags_of_the_vertical_edges_by_height() {
    let tagged = tagged_box();
    let empty = BodyState::empty();
    let datum = Arc::new(build_on(
        json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "15"}),
        &[],
        &[],
    ));

    let output = build_on(
        json!({
            "id": "f-split1", "type": "split_body", "body": "b-1",
            "plane": {"kind": "datum", "feature": "f-dp1"}, "target": "b-2"
        }),
        &[("b-1", &tagged), ("b-2", &empty)],
        &[("f-dp1", &datum)],
    );

    let upper = edge_infos(&state_of(&output, "b-1"));
    let lower = edge_infos(&state_of(&output, "b-2"));
    for edge in VERTICAL_EDGES {
        let below = only_holder(&lower, &format!("{}#1", tag(edge)));
        let above = only_holder(&upper, &format!("{}#2", tag(edge)));
        assert!(
            height(below) < SPLIT_HEIGHT,
            "{} is below the plane",
            below.name
        );
        assert!(
            height(above) > SPLIT_HEIGHT,
            "{} is above the plane",
            above.name
        );
    }
}

#[test]
fn t194_a_split_keeps_the_tags_of_the_horizontal_edges() {
    let tagged = tagged_box();
    let empty = BodyState::empty();
    let datum = Arc::new(build_on(
        json!({"id": "f-dp1", "type": "datum_plane", "base": {"kind": "origin", "plane": "XY"}, "offset": "15"}),
        &[],
        &[],
    ));

    let output = build_on(
        json!({
            "id": "f-split1", "type": "split_body", "body": "b-1",
            "plane": {"kind": "datum", "feature": "f-dp1"}, "target": "b-2"
        }),
        &[("b-1", &tagged), ("b-2", &empty)],
        &[("f-dp1", &datum)],
    );

    // The split numbers the pieces of the side faces. Only the tags stay as they were.
    for (body, edges) in [("b-1", TOP_EDGES), ("b-2", BOTTOM_EDGES)] {
        let infos = edge_infos(&state_of(&output, body));
        for edge in edges {
            only_holder(&infos, &tag(edge));
        }
    }
}

#[test]
fn t195_a_linear_pattern_copy_holds_the_tags_with_its_instance_group() {
    let state = built_on_tagged_box(
        json!({
            "id": "f-lp1", "type": "linear_pattern", "body": "b-1",
            "direction": {"axis": {"kind": "origin", "axis": "X"}, "count": "2", "spacing": "40"},
            "op": "new", "target": "b-2"
        }),
        "b-2",
    );

    assert_copy_tags(&state, "f-lp1");
}

#[test]
fn t195_a_mirror_copy_holds_the_tags_with_its_instance_group() {
    let state = built_on_tagged_box(
        json!({
            "id": "f-mir1", "type": "mirror", "body": "b-1",
            "plane": {"kind": "origin", "plane": "YZ"}, "op": "new", "target": "b-2"
        }),
        "b-2",
    );

    assert_copy_tags(&state, "f-mir1");
}

#[test]
fn t195_a_move_body_keeps_every_tag() {
    let state = built_on_tagged_box(
        json!({"id": "f-mv1", "type": "move_body", "body": "b-1", "translation": ["100", "0", "0"]}),
        "b-1",
    );

    assert_tags_kept(&state, &box_edges(), &[]);
}

/// The description of one stored element of body `b-1`, without a state to read faces from, in a
/// millimeter document whose feature labels are `names`.
fn described(stored: &str, fingerprint_text: &str, names: &LabelNames) -> String {
    let element = name(stored);
    let fingerprint = Fingerprint::parse(fingerprint_text).expect("a valid fingerprint");
    describe_elements(
        &[(&element, &fingerprint)],
        &id("b-1"),
        None,
        None,
        names,
        Units::Mm,
    )
    .remove(0)
}

/// Labels for body `b-1` and a helix `f-hel1` labeled 'Helix 1'.
fn helix_labels() -> LabelNames {
    LabelNames::new(&[("b-1", "Body 1")], &[("f-hel1", "Helix 1", "Helix")])
}

#[test]
fn an_edge_stored_by_its_direct_generator_name_reads_by_its_feature_and_position() {
    assert_eq!(
        described(
            "edge:f-hel1/helix",
            "line|n=1,0,0|c=40,0,30|l=10",
            &helix_labels()
        ),
        "an edge of 'Helix 1' near (40, 0, 30)"
    );
}

#[test]
fn a_vertex_stored_by_its_direct_generator_name_reads_as_a_corner_of_its_feature() {
    assert_eq!(
        described("vertex:f-hel1/start", "point|c=40,0,0", &helix_labels()),
        "a corner of 'Helix 1' near (40, 0, 0)"
    );
}

#[test]
fn a_direct_generator_name_of_a_feature_not_in_the_document_reads_as_missing() {
    let names = LabelNames::new(&[("b-1", "Body 1")], &[]);

    assert_eq!(
        described("edge:f-hel1/helix", "line|n=1,0,0|c=40,0,30|l=10", &names),
        "an edge of a feature that is not in this document"
    );
}
