//! Print analysis: profile, bodies, orientation, checks, bed transforms and overhang maps
//! (engine.md section 3.12).

use std::sync::Arc;

use partsmith_mesh::{Mesh, Transform};
use partsmith_model::{BodyRole, Id};
use partsmith_naming::{ElementKind, Reference};
use partsmith_occt_sys::{GeometryKind, SubShapeKind};
use partsmith_print::{
    AnalysisOptions, CheckKind, OVERLAY_NO_VALUE, PrinterProfile, Rotation, analyze, summarize,
};
use partsmith_protocol::{BodyPrint, DocHandle, Orientation, PrintReport};

use crate::display::{DisplayMesh, MeshKey, kernel_failure};
use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::inspect::{body_with_geometry, shape_of, unique_element};
use crate::profiles::tolerance;
use crate::session::Session;
use crate::state::{BodyView, display_bodies};
use crate::types::{PrintAnalysis, PrintRequest};
use partsmith_protocol::TessellationQuality;

/// The message of a matrix that is not a rotation.
const NOT_A_ROTATION: &str = "The orientation is not a rotation.";

/// The message of a face-down orientation on a curved face.
const NOT_FLAT: &str = "Only flat faces can be laid on the bed.";

impl Session {
    /// Runs the printability checks.
    pub fn analyze_print(
        &mut self,
        doc: DocHandle,
        request: &PrintRequest,
    ) -> Result<PrintAnalysis, EngineError> {
        let open = self.open(doc)?;
        let profile = match &request.profile {
            Some(id) => self
                .profiles
                .find(id)
                .ok_or_else(|| EngineError::UnknownPrinterProfile { id: id.clone() })?,
            None => open.computed.profile.profile.clone(),
        };
        let views = analyzed_bodies(open, request.bodies.as_deref())?;
        let rotation = rotation(open, &request.orientation)?;
        let quality = if self.display {
            open.quality
        } else {
            TessellationQuality::Standard
        };
        let mesh_tolerance = tolerance(&open.computed.profile.profile, quality);
        let mut meshes: Vec<(BodyView, Arc<DisplayMesh>)> = Vec::with_capacity(views.len());
        for view in views {
            let identity = view
                .identity
                .clone()
                .ok_or_else(|| EngineError::internal("an analyzed body has no geometry"))?;
            let mesh = self
                .cache
                .mesh(&MeshKey::new(identity, &mesh_tolerance), &view.state)?;
            meshes.push((view, mesh));
        }
        let transforms = bed_transforms(
            &meshes
                .iter()
                .map(|(_, mesh)| &mesh.mesh)
                .collect::<Vec<_>>(),
            &rotation,
            &profile,
        );
        let options = AnalysisOptions {
            rotation,
            checks: CheckKind::ALL.to_vec(),
        };
        let mut bodies = Vec::with_capacity(meshes.len());
        let mut overhang = Vec::with_capacity(meshes.len());
        for ((view, mesh), transform) in meshes.iter().zip(transforms) {
            let analysis = analyze(&mesh.mesh, &profile, &options)
                .map_err(|error| EngineError::internal(error.to_string()))?;
            let map = if self.display {
                Some(overhang_map(&analysis.overlays, mesh)?)
            } else {
                None
            };
            overhang.push(map);
            bodies.push(BodyPrint {
                body: view.body.clone(),
                transform: transform.to_column_major(),
                mesh_id: None,
                overhang: None,
                report: analysis.report,
            });
        }
        let summary = summarize(&bodies.iter().map(|body| &body.report).collect::<Vec<_>>());
        Ok(PrintAnalysis {
            report: PrintReport {
                profile: profile.id.clone(),
                orientation: request.orientation.clone(),
                summary,
                bodies,
            },
            overhang,
        })
    }
}

/// The bodies to analyze (step 2), in document order: the named ones, checked in request order,
/// or every visible part body with geometry.
fn analyzed_bodies(
    open: &OpenDocument,
    bodies: Option<&[Id]>,
) -> Result<Vec<BodyView>, EngineError> {
    let views = display_bodies(&open.document, &open.computed);
    match bodies {
        Some(bodies) => {
            for body in bodies {
                body_with_geometry(open, body)?;
            }
            Ok(views
                .into_iter()
                .filter(|view| bodies.contains(&view.body))
                .collect())
        }
        None => Ok(views
            .into_iter()
            .filter(|view| {
                view.has_geometry()
                    && open
                        .document
                        .body(&view.body)
                        .is_some_and(|body| body.visible && body.role == BodyRole::Part)
            })
            .collect()),
    }
}

/// The rotation of an orientation (step 3), resolving a face-down face at the display state.
pub(crate) fn rotation(
    open: &OpenDocument,
    orientation: &Orientation,
) -> Result<Rotation, EngineError> {
    let not_a_rotation = |_| EngineError::InvalidOrientation {
        message: NOT_A_ROTATION.to_string(),
    };
    match orientation {
        Orientation::AsModeled => Ok(Rotation::identity()),
        Orientation::Matrix { rows } => Rotation::new(*rows).map_err(not_a_rotation),
        Orientation::FaceDown { body, face } => {
            let normal = flat_face_normal(open, body, face)?;
            Rotation::face_down(normal).map_err(not_a_rotation)
        }
    }
}

/// The outward normal of the flat face a reference resolves to in `body` at the display state.
fn flat_face_normal(
    open: &OpenDocument,
    body: &Id,
    face: &Reference,
) -> Result<[f64; 3], EngineError> {
    let view = body_with_geometry(open, body).map_err(|error| match error {
        EngineError::BodyEmpty { .. } => EngineError::ReferenceUnresolved {
            kind: face.query.kind().to_string(),
        },
        other => other,
    })?;
    let not_flat = || EngineError::InvalidOrientation {
        message: NOT_FLAT.to_string(),
    };
    let (kind, index) = unique_element(&view, face)?;
    if kind != ElementKind::Face {
        return Err(not_flat());
    }
    let geometry = shape_of(&view)?
        .element(SubShapeKind::Face, index)
        .map_err(kernel_failure)?;
    match (geometry.kind, geometry.direction) {
        (GeometryKind::Plane, Some(normal)) => Ok([normal.x, normal.y, normal.z]),
        _ => Err(not_flat()),
    }
}

/// The low and high corners of a box.
type Bounds = ([f64; 3], [f64; 3]);

/// The bed transform of each mesh (step 6): the rotation, then a translation that rests each
/// body on z = 0 and centers the rotated group on the bed.
pub(crate) fn bed_transforms(
    meshes: &[&Mesh],
    rotation: &Rotation,
    profile: &PrinterProfile,
) -> Vec<Transform> {
    let rotated = rotation.to_transform();
    let bounds: Vec<Option<Bounds>> = meshes
        .iter()
        .map(|mesh| {
            mesh.positions()
                .iter()
                .fold(None::<Bounds>, |bounds, &position| {
                    let point = rotated.apply_point(position);
                    Some(match bounds {
                        None => (point, point),
                        Some((low, high)) => (
                            [0, 1, 2].map(|axis| f64::min(low[axis], point[axis])),
                            [0, 1, 2].map(|axis| f64::max(high[axis], point[axis])),
                        ),
                    })
                })
        })
        .collect();
    let union = bounds
        .iter()
        .flatten()
        .fold(None::<Bounds>, |union, &(low, high)| {
            Some(match union {
                None => (low, high),
                Some((union_low, union_high)) => (
                    [0, 1, 2].map(|axis| f64::min(union_low[axis], low[axis])),
                    [0, 1, 2].map(|axis| f64::max(union_high[axis], high[axis])),
                ),
            })
        });
    let center = union.map_or([0.0; 3], |(low, high)| {
        [0, 1, 2].map(|axis| (low[axis] + high[axis]) / 2.0)
    });
    let bed = profile.bed.center();
    bounds
        .iter()
        .map(|bounds| {
            let lowest = bounds.map_or(0.0, |(low, _)| low[2]);
            Transform {
                rows: rotation.rows(),
                translation: [bed[0] - center[0], bed[1] - center[1], -lowest],
            }
        })
        .collect()
}

/// The overhang angle of every display vertex (step 7): the worst angle of the triangles using
/// it, 0 where nothing overhangs.
fn overhang_map(
    overlays: &[partsmith_print::Overlay],
    mesh: &DisplayMesh,
) -> Result<Vec<f32>, EngineError> {
    let vertex_count = mesh.buffers.positions.len() / 3;
    let Some(overlay) = overlays
        .iter()
        .find(|overlay| overlay.info.check == CheckKind::Overhangs)
    else {
        return Ok(vec![0.0; vertex_count]);
    };
    let values = overlay
        .per_vertex(&mesh.buffers.indices, vertex_count)
        .map_err(|error| EngineError::internal(error.to_string()))?;
    Ok(values
        .into_iter()
        .map(|value| {
            if value == OVERLAY_NO_VALUE {
                0.0
            } else {
                value
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use partsmith_print::generic_fdm;

    use super::*;

    fn cube(offset: [f64; 3], size: f64) -> Mesh {
        let corner = |x: f64, y: f64, z: f64| {
            [
                offset[0] + x * size,
                offset[1] + y * size,
                offset[2] + z * size,
            ]
        };
        let positions = vec![
            corner(0.0, 0.0, 0.0),
            corner(1.0, 0.0, 0.0),
            corner(1.0, 1.0, 0.0),
            corner(0.0, 1.0, 0.0),
            corner(0.0, 0.0, 1.0),
            corner(1.0, 0.0, 1.0),
            corner(1.0, 1.0, 1.0),
            corner(0.0, 1.0, 1.0),
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        Mesh::from_triangles(positions, triangles).expect("a valid cube")
    }

    #[test]
    fn an_unrotated_body_is_centered_on_the_bed() {
        let mesh = cube([-10.0, -15.0, 0.0], 20.0);

        let transforms = bed_transforms(&[&mesh], &Rotation::identity(), &generic_fdm());

        assert_eq!(transforms[0].translation, [128.0, 133.0, 0.0]);
        assert_eq!(transforms[0].rows, Rotation::identity().rows());
    }

    #[test]
    fn every_body_rests_on_the_bed() {
        let low = cube([0.0, 0.0, 0.0], 10.0);
        let high = cube([20.0, 0.0, 5.0], 10.0);

        let transforms = bed_transforms(&[&low, &high], &Rotation::identity(), &generic_fdm());

        assert_eq!(transforms[0].translation[2], 0.0);
        assert_eq!(transforms[1].translation[2], -5.0);
        assert_eq!(transforms[0].translation[0], transforms[1].translation[0]);
        assert_eq!(transforms[0].translation[0], 128.0 - 15.0);
    }

    #[test]
    fn upside_down_bodies_rest_on_their_top() {
        let mesh = cube([0.0, 0.0, 0.0], 10.0);
        let flipped = Rotation::new([[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]])
            .expect("a rotation");

        let transforms = bed_transforms(&[&mesh], &flipped, &generic_fdm());

        assert_eq!(transforms[0].translation[2], 10.0);
    }
}
