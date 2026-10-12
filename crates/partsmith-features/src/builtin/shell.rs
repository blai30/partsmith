//! The `shell` feature: hollows a body inward to a wall thickness, optionally opening it by
//! removing faces (features/shell.md).

use partsmith_model::{Category, Expr, FeatureManifest, FieldUnit, Id, SelectionKind};
use partsmith_naming::{ElementKind, FaceSource, NamingMap, Reference, roles};
use partsmith_occt_sys::{KernelError, OpResult, Shape, SubShapeKind, compound, shell};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    body_hint, essential, manifest, positive_length, references, selection_hint, solid_body,
    unit_hint,
};
use super::dressup::{
    FaceOrigins, add_origins, face_names, modified, new_face, run_or_search, smallest_name,
};
use crate::naming::kernel;
use crate::{
    BuildContext, Dependencies, Feature, FeatureError, FeatureOutput, PythonCall, PythonNames,
    PythonValue, name_shape,
};

/// Hollows a body inward to a wall thickness.
#[derive(Clone, Copy, Debug)]
pub struct Shell;

/// Params of a shell feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShellParams {
    /// The body to hollow.
    pub body: Id,
    /// Wall thickness, measured inward. Length, greater than 0.
    pub thickness: Expr,
    /// Faces to remove, opening the shell. Default empty (a closed hollow), omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_faces: Vec<Reference>,
}

impl Feature for Shell {
    const TYPE: &'static str = "shell";
    type Params = ShellParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Shell",
            Category::Modify,
            "shell",
            vec![
                essential(selection_hint(
                    "remove_faces",
                    "Faces to remove",
                    SelectionKind::Face,
                )),
                essential(unit_hint("thickness", "Thickness", FieldUnit::Length)),
                body_hint(),
            ],
        )
    }

    fn dependencies(params: &ShellParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_expression("thickness", &params.thickness, FieldUnit::Length);
        dependencies
    }

    fn build(
        params: &ShellParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, shape) = solid_body(context, &params.body)?;
        let thickness = positive_length(context, "thickness", &params.thickness)?;
        // A shell meant to be open never silently becomes a closed hollow: a list that resolves
        // to nothing is an error.
        let (removed, warnings) = if params.remove_faces.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            let resolved = context.resolve_all(
                "remove_faces",
                &params.body,
                &params.remove_faces,
                ElementKind::Face,
            )?;
            let faces = resolved.elements.iter().map(|face| face.index).collect();
            (faces, resolved.warnings)
        };
        let solids = body_solids(shape, &removed)?;
        let hollow = run_or_search(
            context,
            thickness,
            |thickness| shell_solids(&solids, thickness),
            |max, cause| FeatureError::ShellTooThick {
                field: context.field("thickness"),
                body: params.body.clone(),
                thickness,
                max,
                cause,
            },
        )?;
        let naming = name_shell(
            context.feature(),
            state.naming(),
            &removed,
            &solids,
            &hollow,
        )?;
        Ok(modified(&params.body, hollow.shape, naming, warnings))
    }

    /// `shell(faces, thickness)`: the removed faces as references, or `None` for a closed hollow,
    /// and the thickness as an expression; the generator's `with <part>:` block gives the body.
    fn python(params: &ShellParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let faces = if params.remove_faces.is_empty() {
            PythonValue::None
        } else {
            references(&params.remove_faces)
        };
        let mut call = PythonCall::new("shell");
        call.positional = vec![faces, PythonValue::expression(&params.thickness, names)];
        Some(call)
    }
}

/// One solid of the body to shell, with the body index of each of its elements.
#[derive(Clone, Debug)]
struct BodySolid {
    /// The solid, sharing its elements with the body.
    shape: Shape,
    /// Body index of each face of the solid, in the solid's index order.
    faces: Vec<Option<u32>>,
    /// Body index of each edge of the solid.
    edges: Vec<Option<u32>>,
    /// Body index of each vertex of the solid.
    vertices: Vec<Option<u32>>,
    /// The solid's own indices of its faces to remove.
    remove: Vec<u32>,
}

impl BodySolid {
    /// The body index of element `index` (1-based) of the solid.
    fn body_index(&self, kind: SubShapeKind, index: u32) -> Option<u32> {
        let indices = match kind {
            SubShapeKind::Face => &self.faces,
            SubShapeKind::Edge => &self.edges,
            SubShapeKind::Vertex => &self.vertices,
            SubShapeKind::Solid => return None,
        };
        *indices.get(index.checked_sub(1)? as usize)?
    }
}

/// Each solid of `shape` with the body indices of its elements and the faces of `removed` (body
/// indices) that belong to it.
fn body_solids(shape: &Shape, removed: &[u32]) -> Result<Vec<BodySolid>, FeatureError> {
    let solids = shape.solids().map_err(kernel)?;
    if solids.is_empty() {
        return Err(FeatureError::internal("a body with a shape has no solid"));
    }
    solids
        .into_iter()
        .map(|solid| {
            let locate = |kind| shape.locate(&solid, kind).map_err(kernel);
            let faces = locate(SubShapeKind::Face)?;
            let edges = locate(SubShapeKind::Edge)?;
            let vertices = locate(SubShapeKind::Vertex)?;
            let remove = (1_u32..)
                .zip(&faces)
                .filter(|(_, body)| body.is_some_and(|face| removed.contains(&face)))
                .map(|(face, _)| face)
                .collect();
            Ok(BodySolid {
                shape: solid,
                faces,
                edges,
                vertices,
                remove,
            })
        })
        .collect()
}

/// The shelled solids combined into the body's new shape.
#[derive(Clone, Debug)]
struct Hollow {
    /// The new shape: the one shelled solid, or the compound of several.
    shape: Shape,
    /// The kernel result of each solid, in solid order.
    solids: Vec<OpResult>,
    /// The compound of several shelled solids, whose history input `k` is solid `k`'s result.
    combined: Option<OpResult>,
}

/// Shells each solid with its own faces to remove (shell.md section 3.3 step 4); a solid without
/// any becomes a closed hollow. Several results are combined into one compound (they are
/// disjoint).
fn shell_solids(solids: &[BodySolid], thickness: f64) -> Result<Hollow, KernelError> {
    let results = solids
        .iter()
        .map(|solid| shell(&solid.shape, &solid.remove, thickness))
        .collect::<Result<Vec<_>, _>>()?;
    if let [result] = &results[..] {
        return Ok(Hollow {
            shape: result.shape.clone(),
            solids: results,
            combined: None,
        });
    }
    let shapes: Vec<Shape> = results.iter().map(|result| result.shape.clone()).collect();
    let combined = compound(&shapes)?;
    Ok(Hollow {
        shape: combined.shape.clone(),
        solids: results,
        combined: Some(combined),
    })
}

/// Names the hollow body (naming.md section 3.6.7): the rim where a removed face was is
/// `<feature>/rim/<face>`; kept faces keep their names; an inner offset face, or a join filling
/// the gap at a concave edge or vertex, is `<feature>/inner/<element>`; anything else falls back
/// to `<feature>/face`.
fn name_shell(
    feature: &Id,
    input: &NamingMap,
    removed: &[u32],
    solids: &[BodySolid],
    hollow: &Hollow,
) -> Result<NamingMap, FeatureError> {
    let mut origins = vec![FaceOrigins::default(); hollow.shape.count(SubShapeKind::Face) as usize];
    for (position, (solid, result)) in solids.iter().zip(&hollow.solids).enumerate() {
        let [history] = &result.history.inputs[..] else {
            return Err(FeatureError::internal(
                "a shell result has one history input",
            ));
        };
        let placed = match &hollow.combined {
            None => None,
            Some(combined) => Some(combined.history.inputs.get(position).ok_or_else(|| {
                FeatureError::internal(
                    "the compound of the shelled solids lacks the history of a solid",
                )
            })?),
        };
        let final_face = |face: u32| match placed {
            None => Some(face),
            Some(placed) => placed
                .faces
                .get(face.checked_sub(1)? as usize)?
                .modified
                .first()
                .copied(),
        };
        add_origins(
            &mut origins,
            history,
            |kind, index| solid.body_index(kind, index),
            final_face,
        )?;
    }
    let faces = origins
        .iter()
        .map(|origin| face_source(feature, input, removed, origin))
        .collect::<Result<Vec<_>, FeatureError>>()?;
    name_shape(&hollow.shape, faces, &[input.aliases()])
}

/// The source of one face of the hollow body by the first rule of naming.md section 3.6.7 that
/// applies.
fn face_source(
    feature: &Id,
    input: &NamingMap,
    removed: &[u32],
    origin: &FaceOrigins,
) -> Result<FaceSource, FeatureError> {
    let (rims, kept): (Vec<u32>, Vec<u32>) = origin
        .modified
        .iter()
        .partition(|face| removed.contains(face));
    if let Some(rim) = smallest_name(input, ElementKind::Face, &rims) {
        return new_face(feature, roles::RIM, Some(&rim));
    }
    if !kept.is_empty() {
        return Ok(FaceSource::From(face_names(input, &kept)?));
    }
    let generators = [
        (ElementKind::Face, &origin.faces),
        (ElementKind::Edge, &origin.edges),
        (ElementKind::Vertex, &origin.vertices),
    ];
    for (kind, elements) in generators {
        if let Some(generator) = smallest_name(input, kind, elements) {
            return new_face(feature, roles::INNER, Some(&generator));
        }
    }
    new_face(feature, roles::FACE, None)
}

#[cfg(test)]
mod tests {
    use partsmith_model::FeatureRecord;
    use partsmith_naming::ElementName;
    use partsmith_occt_sys::fuse;
    use serde_json::json;

    use super::*;
    use crate::builtin::dressup::test_fixtures::reference;
    use crate::name_by_history;
    use crate::test_support::ScriptNames;
    use crate::test_support::{face_names as sorted_face_names, id, named_box};

    /// `F-TOP` of shell.md section 6.
    const TOP: &str = r#"{"q": "f-box1/top", "fp": "plane|n=0,0,1|c=0,0,30|a=4000"}"#;

    fn params(thickness: &str, faces: &[&str]) -> ShellParams {
        ShellParams {
            body: id("b-1"),
            thickness: Expr::new(thickness),
            remove_faces: faces.iter().map(|face| reference(face)).collect(),
        }
    }

    fn registry() -> crate::Registry {
        let mut registry = crate::Registry::new();
        registry
            .register::<Shell>()
            .expect("the shell passes the registration checks");
        registry
    }

    /// The index of the face named `name`.
    fn face_index(naming: &NamingMap, name: &str) -> u32 {
        naming
            .index(&ElementName::parse(name).expect("valid name"))
            .expect("the face exists")
    }

    /// Shells `shape` named by `naming` with `removed` faces as the feature `f-sh1` would.
    fn shelled(shape: &Shape, naming: &NamingMap, removed: &[u32]) -> (Hollow, NamingMap) {
        let solids = body_solids(shape, removed).expect("solids");
        let hollow = shell_solids(&solids, 1.0).expect("shells");
        let names = name_shell(&id("f-sh1"), naming, removed, &solids, &hollow).expect("named");
        (hollow, names)
    }

    #[test]
    fn the_manifest_matches_the_spec_table() {
        let registry = registry();

        let manifest = registry.get("shell").expect("registered").manifest();

        assert_eq!(
            serde_json::to_value(manifest).expect("a manifest serializes"),
            json!({
                "label": "Shell", "category": "modify", "icon": "shell", "version": 1,
                "fields": [
                    {"path": "remove_faces", "label": "Faces to remove", "essential": true, "selection": "face"},
                    {"path": "thickness", "label": "Thickness", "essential": true, "unit": "length"},
                    {"path": "body", "label": "Body", "selection": "body"},
                ]
            })
        );
    }

    #[test]
    fn the_feature_type_is_copy_and_debug() {
        fn debug_text<T: Copy + std::fmt::Debug>(value: T) -> String {
            format!("{value:?}")
        }

        assert_eq!(debug_text(Shell), "Shell");
    }

    #[test]
    fn a_closed_hollow_omits_remove_faces() {
        let record: FeatureRecord = serde_json::from_value(json!({
            "id": "f-sh1", "type": "shell", "remove_faces": [], "thickness": "2", "body": "b-1"
        }))
        .expect("a record");

        let validated = registry()
            .validate_and_canonicalize(&record)
            .expect("an empty removal list is valid");

        let keys: Vec<&str> = validated.record.params.keys().map(String::as_str).collect();
        assert_eq!(keys, ["body", "thickness"]);
    }

    #[test]
    fn dependencies_write_the_body_and_read_the_thickness() {
        let dependencies = Shell::dependencies(&params("wall", &[TOP]));

        assert_eq!(dependencies.bodies_written[0].id, id("b-1"));
        assert!(dependencies.bodies_read.is_empty());
        assert_eq!(dependencies.expressions.len(), 1);
        assert_eq!(dependencies.expressions[0].field, "thickness");
        assert_eq!(dependencies.expressions[0].expr, Expr::new("wall"));
        assert_eq!(dependencies.expressions[0].kind, FieldUnit::Length);
    }

    #[test]
    fn python_writes_removed_faces_and_the_thickness() {
        let names = ScriptNames {
            parameters: [("wall".to_string(), "wall".to_string())].into(),
            ..ScriptNames::default()
        };
        let mut call = Shell::python(&params("wall", &[TOP]), &names).expect("a form");
        call.keywords
            .push(("id".to_string(), PythonValue::Str("f-sh1".to_string())));

        assert_eq!(
            call.render(0),
            r#"shell(Ref("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000"), wall, id="f-sh1")"#
        );
    }

    #[test]
    fn python_writes_a_closed_hollow_with_none() {
        let mut call = Shell::python(&params("2", &[]), &ScriptNames::default()).expect("a form");
        call.keywords
            .push(("id".to_string(), PythonValue::Str("f-sh1".to_string())));

        assert_eq!(call.render(0), r#"shell(None, 2, id="f-sh1")"#);
    }

    #[test]
    fn an_open_box_has_a_rim_and_inner_walls() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let top = face_index(&cube.naming, "f-box1/top");

        let (hollow, naming) = shelled(&cube.shape, &cube.naming, &[top]);

        assert_eq!(hollow.shape.count(SubShapeKind::Face), 11);
        assert_eq!(
            sorted_face_names(&naming),
            [
                "f-box1/back",
                "f-box1/bottom",
                "f-box1/front",
                "f-box1/left",
                "f-box1/right",
                "f-sh1/inner/f-box1/back",
                "f-sh1/inner/f-box1/bottom",
                "f-sh1/inner/f-box1/front",
                "f-sh1/inner/f-box1/left",
                "f-sh1/inner/f-box1/right",
                "f-sh1/rim/f-box1/top",
            ]
        );
    }

    #[test]
    fn a_closed_hollow_keeps_every_face_and_adds_one_inner_wall_each() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let (hollow, naming) = shelled(&cube.shape, &cube.naming, &[]);

        assert_eq!(hollow.shape.count(SubShapeKind::Face), 12);
        let names = sorted_face_names(&naming);
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with("f-box1/"))
                .count(),
            6
        );
        assert_eq!(
            names
                .iter()
                .filter(|name| name.starts_with("f-sh1/inner/f-box1/"))
                .count(),
            6
        );
    }

    #[test]
    fn each_solid_of_a_body_is_shelled_with_its_own_faces() {
        let first = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-box2", [30.0, 0.0, 0.0], 10.0);
        let both = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");
        let naming =
            name_by_history(&id("f-box2"), &both, &[&first.naming, &second.naming]).expect("named");
        let tops = [
            face_index(&naming, "f-box1/top"),
            face_index(&naming, "f-box2/top"),
        ];

        let (hollow, names) = shelled(&both.shape, &naming, &tops);

        assert_eq!(hollow.shape.count(SubShapeKind::Solid), 2);
        assert_eq!(hollow.shape.count(SubShapeKind::Face), 22);
        let names = sorted_face_names(&names);
        assert!(names.contains(&"f-sh1/rim/f-box1/top".to_string()));
        assert!(names.contains(&"f-sh1/rim/f-box2/top".to_string()));
        assert!(names.contains(&"f-sh1/inner/f-box2/bottom".to_string()));
        assert!(!names.iter().any(|name| name.starts_with("f-sh1/face")));
    }

    #[test]
    fn a_solid_without_removed_faces_becomes_a_closed_hollow() {
        let first = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);
        let second = named_box("f-box2", [30.0, 0.0, 0.0], 10.0);
        let both = fuse(&first.shape, std::slice::from_ref(&second.shape)).expect("fuses");
        let naming =
            name_by_history(&id("f-box2"), &both, &[&first.naming, &second.naming]).expect("named");
        let top = face_index(&naming, "f-box1/top");

        let solids = body_solids(&both.shape, &[top]).expect("solids");

        let removed: Vec<usize> = solids.iter().map(|solid| solid.remove.len()).collect();
        assert_eq!(removed, [1, 0]);
    }

    #[test]
    fn a_solid_maps_its_elements_to_the_body() {
        let cube = named_box("f-box1", [0.0, 0.0, 0.0], 10.0);

        let solids = body_solids(&cube.shape, &[]).expect("solids");

        assert_eq!(solids.len(), 1);
        for face in 1..=6 {
            assert_eq!(solids[0].body_index(SubShapeKind::Face, face), Some(face));
        }
        assert_eq!(solids[0].body_index(SubShapeKind::Face, 7), None);
        assert_eq!(solids[0].body_index(SubShapeKind::Face, 0), None);
        assert_eq!(solids[0].body_index(SubShapeKind::Solid, 1), None);
    }
}
