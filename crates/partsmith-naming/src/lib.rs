//! Topological naming for Partsmith: element names, references, fingerprints and the resolver.
//!
//! Every face, edge and vertex of a feature output gets an [`ElementName`] derived from how it was
//! made, collected in a [`NamingMap`]. Documents store a [`Reference`] (a name query plus a
//! [`Fingerprint`]) instead of a kernel index, and [`resolve`] finds the referenced elements again
//! after a rebuild. The crate works on plain data ([`TopologyInfo`], [`ElementInfo`]) and never sees
//! kernel shapes. The behavior is specified in `docs/spec/naming.md`.

mod aliases;
mod element_info;
mod error;
mod fingerprint;
mod grammar;
mod kind;
mod map;
mod name;
mod reference;
mod resolver;
pub mod roles;
mod score;
mod topology;

pub use aliases::Aliases;
pub use element_info::ElementInfo;
pub use error::NamingError;
pub use fingerprint::Fingerprint;
pub use kind::{ElementKind, GeometryKind};
pub use map::NamingMap;
pub use name::{ElementName, Generator, Instance};
pub use reference::Reference;
pub use resolver::{
    AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, Candidate, MatchMethod, Resolution, resolve,
};
pub use score::score;
pub use topology::{EdgeInfo, FaceInfo, FaceSource, TopologyInfo, VertexInfo};
