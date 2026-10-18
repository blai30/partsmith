//! Export to STL, 3MF, OBJ and STEP, and STEP import with content-hashed assets (engine.md
//! sections 3.13 and 3.14).

use std::sync::Arc;

use partsmith_io::{
    ExportBody, ExportOutput, IoError, StepExportBody, ThreeMfOptions, read_step, write_3mf,
    write_obj, write_step, write_stl_ascii, write_stl_binary,
};
use partsmith_mesh::{Mesh, TessellationTolerance, Transform};
use partsmith_model::{Body, BodyRole, Edit, FeatureRecord, Id, IdKind};
use partsmith_occt_sys::{Shape, Transform as KernelTransform, transform};
use partsmith_print::Rotation;
use partsmith_protocol::{Diagnostic, DocHandle, ExportFormat, Severity, TessellationQuality};
use serde_json::Value;

use crate::analysis::{bed_transforms, rotation};
use crate::assets::asset_name;
use crate::display::MeshKey;
use crate::error::EngineError;
use crate::ids::NewIds;
use crate::profiles::{is_valid_tolerance, tolerance};
use crate::session::Session;
use crate::state::{self, BodyView, display_bodies, select_bodies};
use crate::types::{ExportRequest, Exported, ExportedFile, Imported};

/// The type name of the import feature.
const IMPORT_TYPE: &str = "import";

/// The name of an imported body when neither the request nor the file names it.
const DEFAULT_IMPORT_NAME: &str = "Imported";

/// One body to export: its record and its state.
struct Exportable {
    /// The body record.
    body: Body,
    /// Its state at the model state.
    view: BodyView,
}

impl Session {
    /// Exports bodies at the model state.
    pub fn export(
        &mut self,
        doc: DocHandle,
        request: &ExportRequest,
    ) -> Result<Exported, EngineError> {
        if let Some(explicit) = &request.tolerance
            && !is_valid_tolerance(explicit)
        {
            return Err(EngineError::InvalidTolerance);
        }
        let open = self.open(doc)?;
        let document = Arc::clone(&open.document);
        let computed = open.computed.clone();
        let assets = open.assets.clone();
        let focused = open.focus().is_some();
        let rotation = request
            .orientation
            .as_ref()
            .map(|orientation| rotation(open, orientation))
            .transpose()?;
        let profile = computed.profile.clone();
        let views = if focused {
            let model = state::run(
                &mut self.recompute,
                &document,
                &assets,
                profile.clone(),
                None,
                0,
            );
            display_bodies(&document, &model)
        } else {
            display_bodies(&document, &computed)
        };
        let three_mf = request.format == ExportFormat::ThreeMf;
        let bodies = exported_bodies(&document, views, request.bodies.as_deref(), three_mf)?;
        let mut warnings = Vec::new();
        let files = match request.format {
            ExportFormat::Step => self.export_step(&bodies, rotation.as_ref(), &profile.profile)?,
            format => {
                let mesh_tolerance = request
                    .tolerance
                    .unwrap_or_else(|| tolerance(&profile.profile, TessellationQuality::Standard));
                let meshes = self.export_meshes(
                    &bodies,
                    &mesh_tolerance,
                    rotation.as_ref(),
                    &profile.profile,
                )?;
                write_meshes(format, &bodies, &meshes, &mut warnings)?
            }
        };
        warnings.extend(profile.diagnostic());
        Ok(Exported { files, warnings })
    }

    /// Imports a STEP file as one new body with one import feature.
    pub fn import_step(
        &mut self,
        doc: DocHandle,
        bytes: Vec<u8>,
        name: Option<&str>,
    ) -> Result<Imported, EngineError> {
        let open = self
            .documents
            .get(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        let step = read_step(&bytes).map_err(|error| EngineError::ImportFailed {
            detail: error.to_string(),
        })?;
        let first = step.solids.first();
        let body_name = name
            .map(str::to_string)
            .or_else(|| {
                first
                    .map(|solid| solid.name.clone())
                    .filter(|name| !name.is_empty())
            })
            .unwrap_or_else(|| DEFAULT_IMPORT_NAME.to_string());
        let color = first.and_then(|solid| solid.color);
        let asset = asset_name(&bytes, "step");
        let body_index = open.document.bodies.len();
        let feature_index = open
            .document
            .rollback
            .as_ref()
            .and_then(|marker| open.document.feature_index(marker))
            .unwrap_or(open.document.features.len());
        let mut ids = NewIds::new(
            &mut *self.ids,
            &open.document,
            self.inspections.with(&self.registry),
        );
        let body_id = ids.next(IdKind::Body)?;
        let feature_id = ids.next(IdKind::Feature)?;
        let mut body = Body::new(body_id.clone(), body_name);
        body.color = color;
        let mut record = FeatureRecord::new(feature_id.clone(), IMPORT_TYPE);
        for (key, value) in [
            ("body", body_id.to_string()),
            ("asset", asset.clone()),
            ("op", "new".to_string()),
        ] {
            record.params.insert(key.to_string(), Value::String(value));
        }
        let edits = vec![
            Edit::AddBody {
                index: body_index,
                body,
            },
            Edit::AddFeature {
                index: feature_index,
                feature: record,
            },
        ];
        let open = self.open_mut(doc)?;
        let stored_before = open.assets.contains_key(&asset);
        open.assets.insert(asset.clone(), Arc::from(bytes));
        if let Err(error) = self.apply_batch(doc, "Import STEP", edits) {
            if !stored_before {
                self.open_mut(doc)?.assets.remove(&asset);
            }
            return Err(error);
        }
        Ok(Imported {
            feature: feature_id,
            body: body_id,
        })
    }

    /// The export meshes of `bodies`, transformed to the bed when a rotation is given.
    fn export_meshes(
        &mut self,
        bodies: &[Exportable],
        mesh_tolerance: &TessellationTolerance,
        rotation: Option<&Rotation>,
        profile: &partsmith_print::PrinterProfile,
    ) -> Result<Vec<Mesh>, EngineError> {
        let mut meshes = Vec::with_capacity(bodies.len());
        for exportable in bodies {
            let identity = exportable
                .view
                .identity
                .clone()
                .ok_or_else(|| EngineError::internal("an exported body has no geometry"))?;
            let display = self.cache.mesh(
                &MeshKey::new(identity, mesh_tolerance),
                &exportable.view.state,
            )?;
            meshes.push(display.mesh.clone());
        }
        let Some(rotation) = rotation else {
            return Ok(meshes);
        };
        let transforms = bed_transforms(&meshes.iter().collect::<Vec<_>>(), rotation, profile);
        meshes
            .iter()
            .zip(&transforms)
            .map(|(mesh, transform)| mesh.transformed(transform).map_err(EngineError::unexpected))
            .collect()
    }

    /// One STEP file with every body, transformed to the bed when a rotation is given.
    fn export_step(
        &mut self,
        bodies: &[Exportable],
        rotation: Option<&Rotation>,
        profile: &partsmith_print::PrinterProfile,
    ) -> Result<Vec<ExportedFile>, EngineError> {
        let mut shapes: Vec<Shape> = Vec::with_capacity(bodies.len());
        for exportable in bodies {
            shapes.push(
                exportable
                    .view
                    .state
                    .shape()
                    .cloned()
                    .ok_or_else(|| EngineError::internal("an exported body has no shape"))?,
            );
        }
        if let Some(rotation) = rotation {
            let standard = tolerance(profile, TessellationQuality::Standard);
            let meshes = self.export_meshes(bodies, &standard, None, profile)?;
            let transforms = bed_transforms(&meshes.iter().collect::<Vec<_>>(), rotation, profile);
            shapes = shapes
                .iter()
                .zip(&transforms)
                .map(|(shape, placement)| {
                    transform(shape, &kernel_transform(placement))
                        .map(|result| result.shape)
                        .map_err(EngineError::unexpected)
                })
                .collect::<Result<_, _>>()?;
        }
        let step_bodies: Vec<StepExportBody<'_>> = bodies
            .iter()
            .zip(&shapes)
            .map(|(exportable, shape)| StepExportBody {
                name: &exportable.body.name,
                color: exportable.body.color,
                shape,
            })
            .collect();
        let bytes = write_step(&step_bodies).map_err(export_failed)?;
        Ok(vec![file("model", "step", bytes)])
    }
}

/// The bodies to export (step 1), in document order.
fn exported_bodies(
    document: &partsmith_model::Document,
    views: Vec<BodyView>,
    requested: Option<&[Id]>,
    three_mf: bool,
) -> Result<Vec<Exportable>, EngineError> {
    let pair = |view: BodyView| {
        document.body(&view.body).map(|body| Exportable {
            body: body.clone(),
            view,
        })
    };
    let bodies: Vec<Exportable> = select_bodies(document, views, requested, |body| {
        body.visible && (body.role == BodyRole::Part || three_mf)
    })?
    .into_iter()
    .filter_map(pair)
    .collect();
    if bodies.is_empty() {
        return Err(EngineError::NothingToExport);
    }
    Ok(bodies)
}

/// Writes the mesh formats (steps 4 to 6), collecting io's warnings.
fn write_meshes(
    format: ExportFormat,
    bodies: &[Exportable],
    meshes: &[Mesh],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<ExportedFile>, EngineError> {
    let export_bodies: Vec<ExportBody<'_>> = bodies
        .iter()
        .zip(meshes)
        .map(|(exportable, mesh)| ExportBody::new(&exportable.body, mesh))
        .collect();
    let mut files = Vec::new();
    let mut add = |name: &str, extension: &str, written: ExportOutput| {
        warnings.extend(written.warnings.into_iter().map(|warning| {
            Diagnostic::new(Severity::Warning, warning.code.as_str(), warning.message)
        }));
        files.push(file(name, extension, written.bytes));
    };
    match format {
        ExportFormat::Stl { binary, per_body } => {
            let write = if binary {
                write_stl_binary
            } else {
                write_stl_ascii
            };
            if per_body {
                let names = file_names(bodies);
                for (body, name) in export_bodies.iter().zip(&names) {
                    add(
                        name,
                        "stl",
                        write(std::slice::from_ref(body)).map_err(export_failed)?,
                    );
                }
            } else {
                add(
                    "model",
                    "stl",
                    write(&export_bodies).map_err(export_failed)?,
                );
            }
        }
        ExportFormat::ThreeMf => {
            let title = match bodies {
                [only] => Some(only.body.name.as_str()),
                _ => None,
            };
            let written =
                write_3mf(&export_bodies, &ThreeMfOptions { title }).map_err(export_failed)?;
            add("model", "3mf", written);
        }
        ExportFormat::Obj => {
            add(
                "model",
                "obj",
                write_obj(&export_bodies).map_err(export_failed)?,
            );
        }
        ExportFormat::Step => {
            return Err(EngineError::internal("STEP is not a mesh format"));
        }
    }
    Ok(files)
}

/// The per-body STL file names (step 5): the body name with characters outside
/// `[A-Za-z0-9 ._-]` replaced by `_` and surrounding spaces trimmed (the body id when nothing is
/// left), with ` (2)`, ` (3)` for repeats.
fn file_names(bodies: &[Exportable]) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(bodies.len());
    for exportable in bodies {
        let cleaned: String = exportable
            .body
            .name
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || " ._-".contains(character) {
                    character
                } else {
                    '_'
                }
            })
            .collect();
        let trimmed = cleaned.trim_matches(' ');
        let base = if trimmed.is_empty() {
            exportable.body.id.to_string()
        } else {
            trimmed.to_string()
        };
        let mut name = base.clone();
        let mut repeat = 1;
        while names.contains(&name) {
            repeat += 1;
            name = format!("{base} ({repeat})");
        }
        names.push(name);
    }
    names
}

/// An exported file named `<name>.<extension>` with the extension's media type.
fn file(name: &str, extension: &str, bytes: Vec<u8>) -> ExportedFile {
    ExportedFile {
        name: format!("{name}.{extension}"),
        media_type: format!("model/{extension}"),
        bytes,
    }
}

/// `export_failed` with the io error's message.
fn export_failed(error: IoError) -> EngineError {
    EngineError::ExportFailed {
        detail: error.to_string(),
    }
}

/// The kernel form of a mesh transform.
fn kernel_transform(placement: &Transform) -> KernelTransform {
    let [x, y, z] = placement.translation;
    let [first, second, third] = placement.rows;
    KernelTransform {
        matrix: [
            [first[0], first[1], first[2], x],
            [second[0], second[1], second[2], y],
            [third[0], third[1], third[2], z],
        ],
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::BodyState;

    use super::*;
    use crate::test_support::id;

    fn exportable(body: &str, name: &str) -> Exportable {
        let body = id(body);
        Exportable {
            body: Body::new(body.clone(), name),
            view: BodyView {
                body,
                state: BodyState::empty(),
                identity: None,
                provider: None,
            },
        }
    }

    #[test]
    fn file_names_are_cleaned_and_numbered() {
        let bodies = [
            exportable("b-1", "Block"),
            exportable("b-2", "Peg/1"),
            exportable("b-3", "Block"),
            exportable("b-4", " é "),
            exportable("b-5", "   "),
        ];

        assert_eq!(
            file_names(&bodies),
            ["Block", "Peg_1", "Block (2)", "_", "b-5"]
        );
    }

    #[test]
    fn files_take_their_media_type_from_the_extension() {
        let exported = file("model", "3mf", vec![1, 2]);

        assert_eq!(exported.name, "model.3mf");
        assert_eq!(exported.media_type, "model/3mf");
        assert_eq!(exported.bytes, [1, 2]);
    }

    #[test]
    fn kernel_transforms_keep_rows_and_translation() {
        let placement = Transform {
            rows: [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [1.0, 2.0, 3.0],
        };

        let kernel = kernel_transform(&placement);

        assert_eq!(kernel.matrix[0], [0.0, -1.0, 0.0, 1.0]);
        assert_eq!(kernel.matrix[2], [0.0, 0.0, 1.0, 3.0]);
    }

    #[test]
    fn nothing_to_export_without_geometry() {
        let document = partsmith_model::Document::default();

        let result = exported_bodies(&document, Vec::new(), None, false);

        assert!(matches!(result, Err(EngineError::NothingToExport)));
    }
}
