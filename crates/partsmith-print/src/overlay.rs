//! Per-triangle color map values and their conversion to per-vertex values (spec section 3.8).

use partsmith_mesh::MAX_ELEMENTS;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::OVERLAY_NO_VALUE;
use crate::error::AnalysisError;
use crate::report::CheckKind;

/// What an overlay measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverlayQuantity {
    /// Overhang angle from vertical in radians; 0 where the triangle does not face down or rests
    /// on the bed.
    OverhangAngle,
    /// Wall thickness in mm; `OVERLAY_NO_VALUE` where it could not be measured.
    WallThickness,
}

/// Which side of the limit is bad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Worse {
    /// Values above the limit are bad.
    Above,
    /// Values below the limit are bad.
    Below,
}

/// Metadata for a color map.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OverlayInfo {
    /// The check the overlay belongs to.
    pub check: CheckKind,
    /// What the values measure.
    pub quantity: OverlayQuantity,
    /// The threshold the colors are centered on.
    pub limit: f64,
    /// Which side of the limit is bad.
    pub worse: Worse,
}

/// Per-triangle values for a viewport color map.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    /// What the values are and how to color them.
    pub info: OverlayInfo,
    /// One value per triangle of the analyzed mesh, in triangle order.
    pub values: Vec<f32>,
}

impl Overlay {
    /// Per-vertex values for an index buffer with 3 entries per triangle in the analyzed mesh's
    /// triangle order (section 3.8).
    ///
    /// Each vertex gets the worst value (largest for `above`, smallest for `below`) among the
    /// triangles that use it and have a value, or `OVERLAY_NO_VALUE` when none has.
    /// `vertex_count` is at most `partsmith_mesh::MAX_ELEMENTS`, like the vertex count of any
    /// mesh.
    pub fn per_vertex(
        &self,
        indices: &[u32],
        vertex_count: usize,
    ) -> Result<Vec<f32>, AnalysisError> {
        if indices.len() != 3 * self.values.len() {
            return Err(AnalysisError::OverlayIndexCount {
                indices: indices.len(),
                triangles: self.values.len(),
            });
        }
        if vertex_count > MAX_ELEMENTS {
            return Err(AnalysisError::OverlayVertexCount { vertex_count });
        }
        if let Some(&index) = indices
            .iter()
            .find(|&&index| index as usize >= vertex_count)
        {
            return Err(AnalysisError::OverlayIndexOutOfRange {
                index,
                vertex_count,
            });
        }
        // Reserving also fails where one allocation cannot hold that many values (32-bit
        // targets), which `vec!` would turn into a panic.
        let mut vertex_values = Vec::new();
        if vertex_values.try_reserve_exact(vertex_count).is_err() {
            return Err(AnalysisError::OverlayVertexCount { vertex_count });
        }
        // A slot still holding `OVERLAY_NO_VALUE` has no value yet, because triangles without
        // a value are skipped.
        vertex_values.resize(vertex_count, OVERLAY_NO_VALUE);
        for (corners, &value) in indices.chunks_exact(3).zip(&self.values) {
            if value == OVERLAY_NO_VALUE {
                continue;
            }
            for &vertex in corners {
                let slot = &mut vertex_values[vertex as usize];
                *slot = if *slot == OVERLAY_NO_VALUE {
                    value
                } else {
                    self.worst(*slot, value)
                };
            }
        }
        Ok(vertex_values)
    }

    /// The worse of two values.
    fn worst(&self, first: f32, second: f32) -> f32 {
        match self.info.worse {
            Worse::Above => first.max(second),
            Worse::Below => first.min(second),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thickness overlay with these values.
    fn thickness(values: Vec<f32>) -> Overlay {
        Overlay {
            info: OverlayInfo {
                check: CheckKind::ThinWalls,
                quantity: OverlayQuantity::WallThickness,
                limit: 0.8,
                worse: Worse::Below,
            },
            values,
        }
    }

    #[test]
    fn unused_vertices_have_no_value() {
        let overlay = thickness(vec![3.0]);
        assert_eq!(
            overlay.per_vertex(&[0, 2, 3], 5),
            Ok(vec![3.0, OVERLAY_NO_VALUE, 3.0, 3.0, OVERLAY_NO_VALUE])
        );
    }

    #[test]
    fn empty_overlay_with_empty_indices_is_valid() {
        let overlay = thickness(Vec::new());
        assert_eq!(overlay.per_vertex(&[], 2), Ok(vec![OVERLAY_NO_VALUE; 2]));
    }

    #[test]
    fn index_count_is_checked_before_index_range() {
        let overlay = thickness(vec![1.0]);
        assert_eq!(
            overlay.per_vertex(&[9, 9], 1),
            Err(AnalysisError::OverlayIndexCount {
                indices: 2,
                triangles: 1
            })
        );
    }

    #[test]
    fn first_offending_index_is_reported() {
        let overlay = thickness(vec![1.0, 1.0]);
        assert_eq!(
            overlay.per_vertex(&[0, 7, 1, 2, 9, 1], 3),
            Err(AnalysisError::OverlayIndexOutOfRange {
                index: 7,
                vertex_count: 3
            })
        );
    }

    #[test]
    fn more_vertices_than_a_mesh_can_have_are_rejected() {
        let overlay = thickness(vec![1.0]);
        assert_eq!(
            overlay.per_vertex(&[0, 0, 0], usize::MAX),
            Err(AnalysisError::OverlayVertexCount {
                vertex_count: usize::MAX
            })
        );
        assert_eq!(
            overlay.per_vertex(&[0, 0, 0], MAX_ELEMENTS + 1),
            Err(AnalysisError::OverlayVertexCount {
                vertex_count: MAX_ELEMENTS + 1
            })
        );
    }

    #[test]
    fn index_count_is_checked_before_vertex_count() {
        let overlay = thickness(vec![1.0]);
        assert_eq!(
            overlay.per_vertex(&[0], usize::MAX),
            Err(AnalysisError::OverlayIndexCount {
                indices: 1,
                triangles: 1
            })
        );
    }

    #[test]
    fn above_takes_the_steepest_overhang() {
        let overlay = Overlay {
            info: OverlayInfo {
                check: CheckKind::Overhangs,
                quantity: OverlayQuantity::OverhangAngle,
                limit: 0.8,
                worse: Worse::Above,
            },
            values: vec![0.0, 1.2, 0.5],
        };
        assert_eq!(
            overlay.per_vertex(&[0, 1, 2, 2, 1, 3, 3, 4, 0], 5),
            Ok(vec![0.5, 1.2, 1.2, 1.2, 0.5])
        );
    }

    #[test]
    fn triangles_without_a_value_do_not_count() {
        let overlay = thickness(vec![OVERLAY_NO_VALUE, 2.0, OVERLAY_NO_VALUE]);
        assert_eq!(
            overlay.per_vertex(&[0, 1, 2, 1, 2, 3, 3, 4, 0], 5),
            Ok(vec![OVERLAY_NO_VALUE, 2.0, 2.0, 2.0, OVERLAY_NO_VALUE])
        );
    }

    #[test]
    fn below_takes_the_thinnest_wall() {
        let overlay = thickness(vec![2.0, 0.6]);
        assert_eq!(
            overlay.per_vertex(&[0, 1, 2, 2, 1, 3], 4),
            Ok(vec![2.0, 0.6, 0.6, 0.6])
        );
    }

    #[test]
    fn info_serializes_with_snake_case_names() {
        let info = thickness(Vec::new()).info;
        assert_eq!(
            serde_json::to_string(&info).expect("overlay infos serialize"),
            r#"{"check":"thin_walls","quantity":"wall_thickness","limit":0.8,"worse":"below"}"#
        );
    }
}
