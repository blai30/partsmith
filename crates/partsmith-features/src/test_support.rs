//! Fixtures shared by the unit tests: ids, a build context over owned inputs, and named boxes.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_model::{Category, FeatureManifest, FieldHint, Id, Units};
use partsmith_naming::{ElementName, FaceSource, NamingMap, roles};
use partsmith_occt_sys::{Plane, Point, PrimitiveFace, Shape, make_box};

use crate::context::{BodyState, BuildContext, BuildInputs, CancelToken, Values};
use crate::naming::name_shape;
use crate::output::{FeatureOutput, Tool};

/// A valid id written in a test.
pub(crate) fn id(text: &str) -> Id {
    Id::new(text).expect("ids written in tests are valid")
}

/// A version 1 manifest labeled "Test" with the given fields.
pub(crate) fn manifest(fields: Vec<FieldHint>) -> FeatureManifest {
    FeatureManifest {
        label: "Test".to_string(),
        category: Category::Create,
        icon: "test".to_string(),
        version: 1,
        fields,
    }
}

/// Owns everything a `BuildContext` borrows. Tests fill the public fields, then call `context`.
pub(crate) struct Fixture {
    /// The feature being built.
    pub(crate) feature: Id,
    /// Its manifest.
    pub(crate) manifest: FeatureManifest,
    /// Values of the declared expressions.
    pub(crate) values: Values,
    /// Declared bodies.
    pub(crate) bodies: BTreeMap<Id, BodyState>,
    /// Outputs of declared features.
    pub(crate) upstream: BTreeMap<Id, Arc<FeatureOutput>>,
    /// Bytes of declared assets.
    pub(crate) assets: BTreeMap<String, Arc<[u8]>>,
    /// Cancellation flag.
    pub(crate) cancel: CancelToken,
}

impl Fixture {
    /// A fixture for feature `feature` with an empty manifest and no inputs.
    pub(crate) fn new(feature: &str) -> Fixture {
        Fixture {
            feature: id(feature),
            manifest: manifest(Vec::new()),
            values: Values::default(),
            bodies: BTreeMap::new(),
            upstream: BTreeMap::new(),
            assets: BTreeMap::new(),
            cancel: CancelToken::new(),
        }
    }

    /// Declares `body` holding `state`.
    pub(crate) fn with_body(mut self, body: &str, state: BodyState) -> Fixture {
        self.bodies.insert(id(body), state);
        self
    }

    /// The context over this fixture's inputs, in a millimeter document.
    pub(crate) fn context(&self) -> BuildContext<'_> {
        BuildContext::new(BuildInputs {
            feature: &self.feature,
            manifest: &self.manifest,
            units: Units::Mm,
            values: &self.values,
            bodies: &self.bodies,
            upstream: &self.upstream,
            assets: &self.assets,
            cancel: &self.cancel,
        })
    }
}

/// An axis-aligned box from `corner` with edge length `size`, its faces named
/// `<feature>/<role>` from the kernel's face roles.
pub(crate) fn named_box(feature: &str, corner: [f64; 3], size: f64) -> Tool {
    let placement = Plane {
        origin: Point::new(corner[0], corner[1], corner[2]),
        ..Plane::XY
    };
    let primitive = make_box(&placement, size, size, size).expect("the box builds");
    let faces = primitive
        .faces
        .iter()
        .map(|face| {
            let role = match face {
                PrimitiveFace::Bottom => roles::BOTTOM,
                PrimitiveFace::Top => roles::TOP,
                PrimitiveFace::Front => roles::FRONT,
                PrimitiveFace::Back => roles::BACK,
                PrimitiveFace::Left => roles::LEFT,
                PrimitiveFace::Right => roles::RIGHT,
                other => panic!("a box has no {other:?} face"),
            };
            FaceSource::New(ElementName::generated(&id(feature), role, None).expect("valid name"))
        })
        .collect();
    let naming = name_shape(&primitive.shape, faces, &[]).expect("the box is named");
    Tool {
        shape: primitive.shape,
        naming,
    }
}

/// A body state holding a named box.
pub(crate) fn box_body(feature: &str, corner: [f64; 3], size: f64) -> BodyState {
    let tool = named_box(feature, corner, size);
    BodyState::new(tool.shape, Arc::new(tool.naming))
}

/// The face names of a naming map, sorted.
pub(crate) fn face_names(naming: &NamingMap) -> Vec<String> {
    let mut names: Vec<String> = naming
        .names(partsmith_naming::ElementKind::Face)
        .map(|(_, name)| name.to_string())
        .collect();
    names.sort_unstable();
    names
}

/// The volume of a shape.
pub(crate) fn volume(shape: &Shape) -> f64 {
    shape.volume().expect("the shape has a volume")
}
