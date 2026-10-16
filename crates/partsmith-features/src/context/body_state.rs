//! The state of a body at one point in the timeline, with its measurements cached (features.md
//! section 2.7).

use std::sync::{Arc, OnceLock};

use partsmith_naming::{
    ElementInfo, ElementName, ElementSet, Motion, NameSet, NamingMap, Successions, TopologyInfo,
};
use partsmith_occt_sys::Shape;

use crate::error::FeatureError;
use crate::naming::{has_element_counts, naming_failed, topology_info};
use crate::output::{BodyUpdate, Measurements};

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
    /// Topology and fingerprints, carried from the update or computed on first use.
    topology: OnceLock<Result<Arc<TopologyInfo>, FeatureError>>,
    /// Element infos, computed on first use.
    elements: OnceLock<Result<Arc<[ElementInfo]>, FeatureError>>,
    /// The resolver's view of the state, computed on first use.
    element_set: OnceLock<Result<Arc<ElementSet>, FeatureError>>,
    /// The names-only view of the naming map, computed on first use.
    name_set: OnceLock<NameSet>,
    /// Volume, carried from the update or computed on first use.
    volume: OnceLock<Result<f64, FeatureError>>,
}

impl BodyState {
    /// A body with no solid.
    pub fn empty() -> Self {
        let naming = NamingMap::build(
            Vec::new(),
            &empty_topology(),
            &[],
            &Successions::default(),
            &Motion::IDENTITY,
        )
        .expect("a shape without elements always has a naming map");
        BodyState::with(None, Arc::new(naming), Measurements::default())
    }

    /// A body holding `shape` named by `naming`.
    pub fn new(shape: Shape, naming: Arc<NamingMap>) -> Self {
        BodyState::with(Some(shape), naming, Measurements::default())
    }

    /// The state a body update gives the body: empty without a shape, else the update's shape and
    /// naming map, with the update's measurements as the values `topology` and `volume` would
    /// compute (features.md section 2.7). Measurements whose topology does not list one entry per
    /// face, edge and vertex of the shape belong to another shape and are measured again instead.
    /// Recompute makes every body state after a feature this way.
    pub fn from_update(update: &BodyUpdate) -> Self {
        match &update.shape {
            None => BodyState::empty(),
            Some(shape) => {
                let measured = if fits(&update.measured, shape) {
                    update.measured.clone()
                } else {
                    Measurements::default()
                };
                BodyState::with(Some(shape.clone()), Arc::clone(&update.naming), measured)
            }
        }
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

    /// Topology and fingerprints of every face, edge and vertex of the shape (`topology_info`,
    /// empty for an empty body), computed once on first use and shared by clones.
    pub fn topology(&self) -> Result<Arc<TopologyInfo>, FeatureError> {
        self.content
            .topology
            .get_or_init(|| match &self.content.shape {
                None => Ok(Arc::new(empty_topology())),
                Some(shape) => topology_info(shape).map(Arc::new),
            })
            .clone()
    }

    /// Element infos (names, kinds, fingerprints, adjacency) of the shape, computed from
    /// `topology` once on first use and shared by clones.
    pub fn elements(&self) -> Result<Arc<[ElementInfo]>, FeatureError> {
        self.content
            .elements
            .get_or_init(|| {
                let topology = self.topology()?;
                let infos = self
                    .content
                    .naming
                    .element_infos(&topology)
                    .map_err(naming_failed)?;
                Ok(Arc::from(infos))
            })
            .clone()
    }

    /// The resolver's view of the state (naming.md section 2.7): `ElementSet::new` of `elements` with the naming
    /// map's aliases and successions, computed once on first use and shared by clones. Empty for an empty body.
    pub fn element_set(&self) -> Result<Arc<ElementSet>, FeatureError> {
        self.content
            .element_set
            .get_or_init(|| {
                let naming = &self.content.naming;
                ElementSet::new(
                    self.elements()?,
                    naming.aliases().clone(),
                    naming.successions().clone(),
                )
                .map(Arc::new)
                .map_err(naming_failed)
            })
            .clone()
    }

    /// The names-only view of the state (naming.md section 2.7): `NameSet::new` of the naming map, computed once on
    /// first use and shared by clones. It reads no topology, so it never computes `elements` (recompute's loss
    /// diagnosis walks writer chains through it, recompute.md section 3.10).
    pub fn name_set(&self) -> &NameSet {
        self.content
            .name_set
            .get_or_init(|| NameSet::new(&self.content.naming))
    }

    /// True when `elements` (or `element_set`, which computes it) has been computed for this state or a clone of
    /// it.
    pub fn elements_computed(&self) -> bool {
        self.content.elements.get().is_some()
    }

    /// The motion the naming map records for a face name (naming.md section 3.3 step 8), also for a name no longer
    /// in the shape; the identity for an empty body.
    pub fn motion(&self, face: &ElementName) -> Motion {
        if self.is_empty() {
            return Motion::IDENTITY;
        }
        self.content.naming.motion(face)
    }

    /// Volume of the shape in cubic millimeters (0 for an empty body), computed once on first use
    /// and shared by clones.
    pub fn volume(&self) -> Result<f64, FeatureError> {
        self.content
            .volume
            .get_or_init(|| match &self.content.shape {
                None => Ok(0.0),
                Some(shape) => Ok(shape.volume()?),
            })
            .clone()
    }

    /// The topology and volume measured so far, for an update that keeps this state: what has not
    /// been computed yet, or failed, is `None`.
    pub fn measurements(&self) -> Measurements {
        Measurements {
            topology: self
                .content
                .topology
                .get()
                .and_then(|topology| topology.as_ref().ok())
                .cloned(),
            volume: self
                .content
                .volume
                .get()
                .and_then(|volume| volume.as_ref().ok())
                .copied(),
        }
    }

    /// A state with this shape and naming map, holding the measured values.
    fn with(shape: Option<Shape>, naming: Arc<NamingMap>, measured: Measurements) -> Self {
        BodyState {
            content: Arc::new(BodyContent {
                shape,
                naming,
                topology: known(measured.topology.map(Ok)),
                elements: OnceLock::new(),
                element_set: OnceLock::new(),
                name_set: OnceLock::new(),
                volume: known(measured.volume.map(Ok)),
            }),
        }
    }
}

/// True unless `measured` carries a topology whose face, edge or vertex count differs from the
/// shape's.
fn fits(measured: &Measurements, shape: &Shape) -> bool {
    measured.topology.as_ref().is_none_or(|topology| {
        has_element_counts(
            shape,
            topology.faces.len(),
            topology.edges.len(),
            topology.vertices.len(),
        )
    })
}

/// A cell holding `value` when it is known, else an empty one.
fn known<T>(value: Option<T>) -> OnceLock<T> {
    value.map_or_else(OnceLock::new, OnceLock::from)
}

/// The topology of a shape without elements.
fn empty_topology() -> TopologyInfo {
    TopologyInfo {
        faces: Vec::new(),
        edges: Vec::new(),
        vertices: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{box_body, id};

    #[test]
    fn empty_bodies_have_no_elements() {
        let state = BodyState::empty();

        assert!(state.is_empty());
        assert!(state.shape().is_none());
        assert_eq!(state.elements().expect("no elements").len(), 0);
    }

    #[test]
    fn empty_bodies_have_no_volume() {
        assert_eq!(BodyState::empty().volume().expect("no volume"), 0.0);
    }

    #[test]
    fn the_volume_of_a_state_is_shared_by_its_clones() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let clone = state.clone();

        let volume = state.volume().expect("the box has a volume");

        assert!((volume - 1000.0).abs() < 1e-9, "{volume}");
        assert_eq!(clone.volume().expect("the clone has a volume"), volume);
    }

    #[test]
    fn the_topology_of_a_state_is_the_kernel_topology_and_shared_by_its_clones() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let clone = state.clone();

        let topology = state.topology().expect("the box has a topology");

        assert_eq!(topology.faces.len(), 6);
        let shape = state.shape().expect("a solid");
        assert_eq!(
            *topology,
            topology_info(shape).expect("the kernel topology")
        );
        assert!(Arc::ptr_eq(
            &topology,
            &clone.topology().expect("the clone has a topology")
        ));
    }

    #[test]
    fn the_name_set_of_a_state_reads_names_only_and_is_shared_by_its_clones() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let clone = state.clone();
        let top = ElementName::parse("f-box1/top").expect("a face name");
        let front = ElementName::parse("f-box1/front").expect("a face name");

        let names = state.name_set();

        assert!(names.touches(&top, &front));
        assert!(std::ptr::eq(names, clone.name_set()));
        assert!(!state.elements_computed());
        assert!(state.measurements().topology.is_none());
    }

    #[test]
    fn a_state_measures_nothing_until_asked() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);

        let before = state.measurements();
        state.volume().expect("a volume");
        state.elements().expect("element infos");
        let after = state.measurements();

        assert!(before.topology.is_none() && before.volume.is_none());
        assert!(after.topology.is_some());
        assert_eq!(after.volume, Some(state.volume().expect("a volume")));
    }

    #[test]
    fn a_state_from_an_update_keeps_its_measurements() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let shape = state.shape().cloned().expect("the box is a solid");
        let topology = Arc::new(crate::topology_info(&shape).expect("a topology"));
        let volume = shape.volume().expect("a volume");
        let update = BodyUpdate {
            body: id("b-1"),
            shape: Some(shape),
            naming: Arc::clone(state.naming()),
            change: crate::output::BodyChange::Modify,
            measured: Measurements {
                topology: Some(Arc::clone(&topology)),
                volume: Some(volume),
            },
        };

        let next = BodyState::from_update(&update);

        // The carried values are cached before anything asks for them.
        let cached = next.measurements();
        assert!(
            cached
                .topology
                .is_some_and(|carried| Arc::ptr_eq(&carried, &topology))
        );
        assert_eq!(cached.volume, Some(volume));
        assert!(Arc::ptr_eq(next.naming(), state.naming()));
    }

    #[test]
    fn a_state_from_an_update_measures_again_what_another_shape_gave() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let cylinder = partsmith_occt_sys::make_cylinder(&partsmith_occt_sys::Plane::XY, 5.0, 10.0)
            .expect("a cylinder")
            .shape;
        let update = BodyUpdate {
            body: id("b-1"),
            shape: state.shape().cloned(),
            naming: Arc::clone(state.naming()),
            change: crate::output::BodyChange::Modify,
            // The cylinder's topology, as a plugin copying another state's measurements would.
            measured: Measurements {
                topology: Some(Arc::new(
                    crate::topology_info(&cylinder).expect("a topology"),
                )),
                volume: Some(cylinder.volume().expect("a volume")),
            },
        };

        let next = BodyState::from_update(&update);

        let cached = next.measurements();
        assert!(cached.topology.is_none() && cached.volume.is_none());
        assert_eq!(next.topology().expect("a topology").faces.len(), 6);
        assert!((next.volume().expect("a volume") - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn a_state_from_an_update_without_measurements_measures_the_shape() {
        let state = box_body("f-box1", [0.0, 0.0, 0.0], 10.0);
        let update = BodyUpdate {
            body: id("b-1"),
            shape: state.shape().cloned(),
            naming: Arc::clone(state.naming()),
            change: crate::output::BodyChange::Modify,
            measured: Measurements::default(),
        };

        let next = BodyState::from_update(&update);

        assert!((next.volume().expect("a volume") - 1000.0).abs() < 1e-9);
        assert_eq!(next.topology().expect("a topology").faces.len(), 6);
    }

    #[test]
    fn a_state_from_an_update_without_a_shape_is_empty() {
        let update = BodyUpdate {
            body: id("b-1"),
            shape: None,
            naming: Arc::clone(BodyState::empty().naming()),
            change: crate::output::BodyChange::Clear,
            measured: Measurements {
                topology: None,
                volume: Some(5.0),
            },
        };

        let next = BodyState::from_update(&update);

        assert!(next.is_empty());
        assert_eq!(next.volume().expect("no volume"), 0.0);
        assert!(next.topology().expect("no elements").faces.is_empty());
    }
}
