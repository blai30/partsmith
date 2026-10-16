//! Fixtures the build context's unit tests share: bodies, stored references and sketch outputs.

use std::sync::Arc;

use partsmith_naming::{
    ElementKind, ElementName, FaceSource, Fingerprint, Motion, Reference, Successions,
};
use partsmith_sketch::{OriginPlane, Placement, SketchEntity, build_profile};
use serde_json::json;

use super::BodyState;
use crate::output::{FeatureOutput, OutputData, SketchResult};
use crate::test_support::{Fixture, box_body, id};

/// A stored reference of `query` with `fingerprint`.
pub(super) fn reference(query: &str, fingerprint: &str) -> Reference {
    Reference::new(
        ElementName::parse(query).expect("valid name"),
        Fingerprint::parse(fingerprint).expect("valid fingerprint"),
    )
    .expect("the kinds agree")
}

/// A far-away fingerprint that matches nothing.
pub(super) const LOST_FACE: &str = "plane|n=0,0,1|c=500,500,500|a=1";

/// A fixture whose `b-1` is a 10 mm box with its corner at the origin.
pub(super) fn box_fixture() -> Fixture {
    Fixture::new("f-test1").with_body("b-1", box_body("f-box1", [0.0, 0.0, 0.0], 10.0))
}

/// A fixture whose `b-1` is a 10 mm box with its corner at the origin whose faces have the
/// fallback names of imported faces, `f-imp1/face#1` to `#6`, numbered by centroid x, then y,
/// then z (naming.md sections 3.4 and 3.6.14), so the top is `#4`.
pub(super) fn imported_box_fixture() -> Fixture {
    let block = crate::test_support::named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
    let face = ElementName::parse("f-imp1/face").expect("valid name");
    let faces = (0..block.naming.count(ElementKind::Face))
        .map(|_| FaceSource::New(face.clone()))
        .collect();
    let named = crate::naming::name_shape(
        &block.shape,
        faces,
        &[],
        &Successions::default(),
        &Motion::IDENTITY,
    )
    .expect("the box is named");
    Fixture::new("f-test1").with_body("b-1", BodyState::new(block.shape, Arc::new(named.naming)))
}

/// The output of a sketch on `placement` with these entities.
pub(super) fn sketch_output(
    placement: Placement,
    entities: serde_json::Value,
) -> Arc<FeatureOutput> {
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
    Arc::new(FeatureOutput::new(
        OutputData::Sketch(SketchResult {
            placement,
            motion: Motion::IDENTITY,
            profile: build_profile(&entities),
            solved: partsmith_sketch::SolveResult { entities, ..solved },
        }),
        Vec::new(),
    ))
}

/// The rectangle (0, 0)-(80, 50) of lines e-1 to e-4, plus a circle e-5 of radius 10 at its center.
pub(super) fn rectangle_with_circle() -> serde_json::Value {
    json!([
        {"id": "e-1", "type": "line", "start": [0, 0], "end": [80, 0]},
        {"id": "e-2", "type": "line", "start": [80, 0], "end": [80, 50]},
        {"id": "e-3", "type": "line", "start": [80, 50], "end": [0, 50]},
        {"id": "e-4", "type": "line", "start": [0, 50], "end": [0, 0]},
        {"id": "e-5", "type": "circle", "center": [40, 25], "radius": 10},
        {"id": "e-9", "type": "line", "start": [0, 0], "end": [0, 10], "construction": true}
    ])
}

/// A fixture whose sketch `f-sk1` on XY has `rectangle_with_circle`.
pub(super) fn sketch_fixture() -> Fixture {
    let mut fixture = Fixture::new("f-ext1");
    fixture.upstream.insert(
        id("f-sk1"),
        sketch_output(OriginPlane::Xy.placement(), rectangle_with_circle()),
    );
    fixture
}

/// A fingerprint that six faces of a 10 mm box match equally: only the kind and the area.
pub(super) const ANY_BOX_FACE: &str = "plane|a=100";

/// An imported face that is gone. Fallback names are confirmed by their fingerprint, so with
/// `ANY_BOX_FACE` every face of `imported_box_fixture` fits it equally (naming.md section 3.9
/// step 1); an unknown name of any other face never resolves by its fingerprint.
pub(super) const GONE_IMPORTED_FACE: &str = "f-imp1/face#9";
