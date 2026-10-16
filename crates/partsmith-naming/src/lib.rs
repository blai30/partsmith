//! Topological naming for Partsmith: element names, references, fingerprints and the resolver.
//!
//! Every face, edge and vertex of a feature output gets an [`ElementName`] derived from how it was
//! made, collected in a [`NamingMap`] with the aliases of merged faces, the successions of replaced
//! sketch entities and the motions of moved faces. Documents store a [`Reference`] (a name query, a
//! [`Fingerprint`], and what tells the element apart from its siblings) instead of a kernel index,
//! and [`resolve`] finds the referenced elements again after a rebuild by meaning, never by a
//! fingerprint guess. The crate works on plain data ([`TopologyInfo`], [`ElementInfo`],
//! [`ElementSet`]) and never sees kernel shapes. The behavior is specified in
//! `docs/spec/naming.md`.

mod aliases;
mod canon;
mod chase;
mod element_info;
mod element_set;
mod error;
mod face_name;
mod fast_hash;
mod fingerprint;
mod grammar;
mod kind;
mod map;
mod motion;
mod name;
mod name_set;
mod pick;
mod reference;
mod resolver;
pub mod roles;
mod score;
mod shape;
#[cfg(test)]
mod testing;
mod topology;

pub use aliases::{Aliases, Successions};
pub use element_info::ElementInfo;
pub use element_set::ElementSet;
pub use error::NamingError;
pub use face_name::{CopyGroup, FaceGenerator, FaceName};
pub use fingerprint::Fingerprint;
pub use kind::{ElementKind, GeometryKind};
pub use map::NamingMap;
pub use motion::Motion;
pub use name::{ElementName, Generator, GeneratorUse, Instance};
pub use name_set::NameSet;
pub use reference::{ANCHOR_LIMIT, Anchors, Merged, MergedName, Reference};
pub use resolver::{
    AMBIGUITY_MARGIN, CONFIDENCE_THRESHOLD, Candidate, Follow, Lost, MatchMethod, Resolution,
    resolve,
};
pub use score::score;
pub use shape::piece_free_text;
pub use topology::{EdgeInfo, FaceInfo, FaceSource, TopologyInfo, VertexInfo};

/// True when `items` is non-empty and strictly ascending: a sorted list of distinct items, as alias
/// targets, anchors and merged names are stored.
pub(crate) fn is_sorted_list<T: Ord>(items: &[T]) -> bool {
    !items.is_empty() && items.windows(2).all(|pair| pair[0] < pair[1])
}
