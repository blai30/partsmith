//! Errors from building or parsing names, fingerprints, references and naming maps (naming.md section 4).

use crate::kind::{ElementKind, GeometryKind};

/// Errors from building or parsing names, fingerprints, references and naming maps.
///
/// They indicate a programming error in a feature or a corrupt document; the feature or loader that
/// receives one reports it as its own error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NamingError {
    /// A text is not a valid element name.
    #[error("\"{text}\" is not a valid element name: {reason}.")]
    InvalidName {
        /// The rejected text.
        text: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A role is not a lowercase word.
    #[error(
        "\"{role}\" is not a valid role: use lowercase letters, digits and hyphens, starting with a letter."
    )]
    InvalidRole {
        /// The rejected role.
        role: String,
    },
    /// A sketch generator is not an entity id with an optional role.
    #[error(
        "\"{text}\" is not a valid sketch reference: use an entity id, optionally followed by a dot and a role, such as e-1.center."
    )]
    InvalidGenerator {
        /// The rejected sketch reference.
        text: String,
    },
    /// Instance `(0, 0)` was asked for an instance name.
    #[error("Instance 0 is the original and has no instance name.")]
    InvalidInstance,
    /// A constructed name exceeds the length or nesting limit.
    #[error(
        "An element name would be {length} bytes long or nested too deeply; the limits are 16384 bytes and 32 levels."
    )]
    NameTooLong {
        /// Length of the name that was too long or too deep, in bytes.
        length: usize,
    },
    /// A text is not a valid fingerprint.
    #[error("\"{text}\" is not a valid fingerprint: {reason}.")]
    InvalidFingerprint {
        /// The rejected text.
        text: String,
        /// Why it was rejected.
        reason: String,
    },
    /// A reference's name and fingerprint denote different kinds of element.
    #[error(
        "The reference \"{name}\" names a different kind of element than its fingerprint ({kind})."
    )]
    KindMismatch {
        /// The reference's element name.
        name: String,
        /// The fingerprint's geometry kind.
        kind: GeometryKind,
    },
    /// The number of face sources differs from the number of faces.
    #[error("Naming received {sources} face sources for a shape with {faces} faces.")]
    FaceCountMismatch {
        /// Number of face sources.
        sources: usize,
        /// Number of faces in the topology.
        faces: usize,
    },
    /// A face source lists no input names.
    #[error("Face {face} has no source names.")]
    EmptySources {
        /// The 1-based face index.
        face: u32,
    },
    /// An element index is 0 or larger than the number of elements of its kind.
    #[error("{kind} index {index} is out of range: the shape has {count}.")]
    IndexOutOfRange {
        /// Kind of the indexed element.
        kind: ElementKind,
        /// The rejected 1-based index.
        index: u32,
        /// Number of elements of that kind.
        count: u32,
    },
    /// An adjacency list is not strictly ascending.
    #[error("The adjacency list of {kind} {index} is not ascending and distinct.")]
    UnsortedIndices {
        /// Kind of the element that owns the list.
        kind: ElementKind,
        /// The element's 1-based index.
        index: u32,
    },
    /// Elements sharing a name cannot be ordered because a fingerprint has no centroid.
    #[error("The fingerprint of {kind} {index} has no centroid, so it cannot be ordered.")]
    MissingCentroid {
        /// Kind of the element.
        kind: ElementKind,
        /// The element's 1-based index.
        index: u32,
    },
    /// A topology's element counts differ from a naming map's.
    #[error("The topology does not match the naming map's element counts.")]
    TopologyMismatch,
    /// A naming map lists a name twice.
    #[error("The naming map lists \"{name}\" more than once.")]
    DuplicateName {
        /// The repeated name.
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_name_message_quotes_text_and_reason() {
        let error = NamingError::InvalidName {
            text: "f-a".to_owned(),
            reason: "it ends too early".to_owned(),
        };

        assert_eq!(
            error.to_string(),
            "\"f-a\" is not a valid element name: it ends too early."
        );
    }

    #[test]
    fn index_out_of_range_message_names_the_kind() {
        let error = NamingError::IndexOutOfRange {
            kind: ElementKind::Face,
            index: 7,
            count: 6,
        };

        assert_eq!(
            error.to_string(),
            "face index 7 is out of range: the shape has 6."
        );
    }

    #[test]
    fn kind_mismatch_message_names_the_geometry_kind() {
        let error = NamingError::KindMismatch {
            name: "edge(f-a/x)".to_owned(),
            kind: GeometryKind::BsplineSurface,
        };

        assert_eq!(
            error.to_string(),
            "The reference \"edge(f-a/x)\" names a different kind of element than its fingerprint (bspline_surface)."
        );
    }

    #[test]
    fn name_too_long_message_states_the_limits() {
        let error = NamingError::NameTooLong { length: 16385 };

        assert_eq!(
            error.to_string(),
            "An element name would be 16385 bytes long or nested too deeply; the limits are 16384 bytes and 32 levels."
        );
    }
}
