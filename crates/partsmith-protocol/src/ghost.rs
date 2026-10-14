//! Geometry drawn where something was: a lost element in its last build, a lost region's outline,
//! or a sketch entity before an edit (protocol.md sections 2.4 and 3.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::BufferIndex;

/// Geometry drawn where something was: a lost element in its last build, a lost region's outline, or a sketch entity before an edit. Positions travel as a binary buffer of the same frame (section 3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Ghost {
    /// `f32` positions in mm: 3 per point in `world`, 2 per point (u, v) in `sketch`.
    pub positions: BufferIndex,
    /// How the points are joined.
    pub kind: GhostKind,
    /// The coordinates of `positions`.
    pub frame: GhostFrame,
}

/// How the points of a ghost are joined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GhostKind {
    /// Line segments, 2 points each (an edge, a region outline, a sketch curve).
    Lines,
    /// Triangles, 3 points each, counterclockwise seen from outside (a face).
    Triangles,
    /// Separate points (a vertex, a sketch point).
    Points,
}

/// The coordinates of a ghost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GhostFrame {
    /// Model coordinates.
    World,
    /// The coordinates of the sketch the ghost belongs to: a region repair's `sketch`, or a cause's `feature`.
    Sketch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_writes_its_buffer_kind_and_frame() {
        let ghost = Ghost {
            positions: BufferIndex(3),
            kind: GhostKind::Triangles,
            frame: GhostFrame::World,
        };

        let text = serde_json::to_string(&ghost).expect("a ghost serializes");

        assert_eq!(
            text,
            r#"{"positions":3,"kind":"triangles","frame":"world"}"#
        );
    }

    #[test]
    fn ghost_ignores_unknown_fields() {
        let ghost: Ghost =
            serde_json::from_str(r#"{"positions":0,"kind":"points","frame":"sketch","count":1}"#)
                .expect("a ghost with an unknown field deserializes");

        assert_eq!(ghost.kind, GhostKind::Points);
    }
}
