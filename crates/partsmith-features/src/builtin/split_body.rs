//! The `split_body` feature: cuts a body in two with a plane, keeps the part on one side and moves
//! the other part to another, empty body or discards it (docs/spec/features/split_body.md).

use std::sync::Arc;

use partsmith_model::{Category, FeatureManifest, Id, SelectionKind};
use partsmith_naming::{FaceSource, NamingMap, roles};
use partsmith_occt_sys::{PlaneSide, Shape, SplitResult, SubShapeKind, split};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    bodies, essential, face_name, flag, hint, is_false, manifest, naming_failed, part,
    selection_hint, single_or_compound, solid_body, target_hint,
};
use crate::context::BuildContext;
use crate::deps::Dependencies;
use crate::error::{FeatureError, FieldRef};
use crate::feature::Feature;
use crate::output::{BodyChange, BodyUpdate, FeatureOutput, Measurements, VOLUME_TOLERANCE};
use crate::params::{PlacementExt, PlaneRef};
use crate::python::{PythonCall, PythonNames, PythonValue};
use crate::{modified_sources, name_shape};

/// Cuts a body in two with a plane.
#[derive(Clone, Copy, Debug, Default)]
pub struct SplitBody;

/// Params of a split body feature.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SplitBodyParams {
    /// The body to split. It keeps the front part.
    pub body: Id,
    /// The split plane.
    pub plane: PlaneRef,
    /// Keep the back part instead of the front part. Default false, omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip: bool,
    /// The empty body that receives the other part. Omitted (None) discards it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Id>,
}

impl Feature for SplitBody {
    const TYPE: &'static str = "split_body";
    type Params = SplitBodyParams;

    fn manifest() -> FeatureManifest {
        manifest(
            "Split body",
            Category::Modify,
            "split-body",
            vec![
                essential(selection_hint("body", "Body", SelectionKind::Body)),
                essential(selection_hint("plane", "Split plane", SelectionKind::Plane)),
                essential(target_hint()),
                essential(hint("flip", "Flip")),
            ],
        )
    }

    /// The target is not the split body (split_body.md section 3.1).
    fn validate(params: &SplitBodyParams) -> Result<(), FeatureError> {
        match &params.target {
            Some(target) if *target == params.body => Err(FeatureError::SameBody {
                field: FieldRef::new("target", &SplitBody::manifest()),
                body: target.clone(),
            }),
            _ => Ok(()),
        }
    }

    /// Writes the body, then the target when set; reads what the plane reads (split_body.md
    /// section 3.2).
    fn dependencies(params: &SplitBodyParams) -> Dependencies {
        let mut dependencies = Dependencies::default();
        dependencies.add_body_written(&params.body, "body");
        dependencies.add_plane("plane", &params.plane);
        if let Some(target) = &params.target {
            dependencies.add_body_written(target, "target");
        }
        dependencies
    }

    /// Splits the body by the plane, names the whole result once, then gives the body the
    /// solids on the side it keeps and the target the others, each named by the projection of
    /// the whole map (split_body.md section 3.3).
    fn build(
        params: &SplitBodyParams,
        context: &BuildContext<'_>,
    ) -> Result<FeatureOutput, FeatureError> {
        let (state, shape) = solid_body(context, &params.body)?;
        let plane = context.plane("plane", &params.plane)?;
        let pieces = split(shape, &plane.to_kernel())?;
        // A side holds no solid when its volume is at most the volume tolerance of the body
        // (split_body.md section 3.3).
        let threshold = VOLUME_TOLERANCE * state.volume()?;
        let (positive, negative) = sides(&pieces, threshold)?;
        if !positive.reached || !negative.reached {
            return Err(FeatureError::PlaneMissesBody {
                body: params.body.clone(),
            });
        }
        if let Some(target) = &params.target
            && !context.body(target)?.is_empty()
        {
            return Err(FeatureError::BodyNotEmpty {
                body: target.clone(),
            });
        }
        let whole = name_pieces(context, &pieces, state.naming())?;
        let (kept, other) = if params.flip {
            (negative, positive)
        } else {
            (positive, negative)
        };
        let result = &pieces.result.shape;
        let mut updates = vec![part_update(
            &params.body,
            result,
            &whole,
            kept.solids,
            BodyChange::Modify,
        )?];
        if let Some(target) = &params.target {
            updates.push(part_update(
                target,
                result,
                &whole,
                other.solids,
                BodyChange::New,
            )?);
        }
        Ok(bodies(updates, Vec::new()))
    }

    /// `split(plane, into=<target part>, flip=True)` (split_body.md section 2): `into` when a
    /// target is set (without it the other part is discarded), `flip` when set. None when the
    /// plane or the target has no variable.
    fn python(params: &SplitBodyParams, names: &dyn PythonNames) -> Option<PythonCall> {
        let mut call = PythonCall::new("split");
        call.positional
            .push(PythonValue::plane(&params.plane, names)?);
        if let Some(target) = &params.target {
            call.keywords
                .push(("into".to_string(), part(names, target)?));
        }
        if params.flip {
            call.keywords.push(flag("flip"));
        }
        Some(call)
    }
}

/// The solids of a split result on one side of the plane.
#[derive(Debug, Default)]
struct Side {
    /// The solids, in solid index order.
    solids: Vec<Shape>,
    /// True when one of them has a volume above the threshold, so the plane really cuts the body
    /// on this side.
    reached: bool,
}

/// The solids of `pieces` on the positive and on the negative side of the plane.
fn sides(pieces: &SplitResult, threshold: f64) -> Result<(Side, Side), FeatureError> {
    let solids = pieces.result.shape.solids()?;
    if solids.len() != pieces.solid_sides.len() {
        return Err(FeatureError::internal(
            "the kernel gave a side for each solid of a split, but the counts differ",
        ));
    }
    let mut positive = Side::default();
    let mut negative = Side::default();
    for (solid, side) in solids.into_iter().zip(&pieces.solid_sides) {
        let reaches = solid.volume()? > threshold;
        let side = match side {
            PlaneSide::Positive => &mut positive,
            PlaneSide::Negative => &mut negative,
        };
        side.reached |= reaches;
        side.solids.push(solid);
    }
    Ok((positive, negative))
}

/// Names every face of the whole split result (naming.md section 3.6.13): a face descending from
/// input faces keeps their names, a section face is `<id>/split-positive` or `<id>/split-negative`
/// by the side of the piece it bounds, and anything else gets the fallback `<id>/face`. Pieces of
/// one face on both sides are numbered together.
fn name_pieces(
    context: &BuildContext<'_>,
    pieces: &SplitResult,
    input: &NamingMap,
) -> Result<NamingMap, FeatureError> {
    let feature = context.feature();
    let positive = face_name(feature, roles::SPLIT_POSITIVE, None)?;
    let negative = face_name(feature, roles::SPLIT_NEGATIVE, None)?;
    let fallback = face_name(feature, roles::FACE, None)?;
    let faces = modified_sources(&pieces.result, &[input])?
        .into_iter()
        .zip(1..)
        .map(|(sources, face)| {
            if !sources.is_empty() {
                return FaceSource::From(sources);
            }
            let section = pieces
                .section_faces
                .binary_search_by_key(&face, |section| section.face)
                .ok()
                .map(|position| pieces.section_faces[position].side);
            FaceSource::New(match section {
                Some(PlaneSide::Positive) => positive.clone(),
                Some(PlaneSide::Negative) => negative.clone(),
                None => fallback.clone(),
            })
        })
        .collect();
    Ok(name_shape(&pieces.result.shape, faces, &[input.aliases()])?.naming)
}

/// The update giving `body` these solids of the split `result`: one solid as it is, several as
/// a compound, named by the projection of the whole result's map (naming.md section 3.5.2).
fn part_update(
    body: &Id,
    result: &Shape,
    whole: &NamingMap,
    solids: Vec<Shape>,
    change: BodyChange,
) -> Result<BodyUpdate, FeatureError> {
    let shape = single_or_compound(solids)?;
    let indices = |kind: SubShapeKind| -> Result<Vec<u32>, FeatureError> {
        result
            .locate(&shape, kind)?
            .into_iter()
            .collect::<Option<Vec<u32>>>()
            .ok_or_else(|| {
                FeatureError::internal("a piece of a split has an element the split result lacks")
            })
    };
    let naming = whole
        .project(
            &indices(SubShapeKind::Face)?,
            &indices(SubShapeKind::Edge)?,
            &indices(SubShapeKind::Vertex)?,
        )
        .map_err(naming_failed)?;
    Ok(BodyUpdate {
        body: body.clone(),
        shape: Some(shape),
        naming: Arc::new(naming),
        change,
        measured: Measurements::default(),
    })
}

#[cfg(test)]
mod tests {
    use partsmith_naming::ElementName;
    use partsmith_occt_sys::GeometryKind;
    use serde_json::{Value, json};

    use super::*;
    use crate::context::BodyState;
    use crate::output::{DatumResult, FeatureOutput, OutputData};
    use crate::test_support::{
        assert_bounds, assert_volume, box_body, build, face_names, id, params_with, python, update,
        updated_shape, updates,
    };

    fn split_body(plane: Value, extra: Value) -> SplitBodyParams {
        params_with::<SplitBody>(
            json!({"id": "f-split1", "type": "split_body", "body": "b-1", "plane": plane}),
            extra,
        )
    }

    fn datum() -> Value {
        json!({"kind": "datum", "feature": "f-dp1"})
    }

    /// The datum plane parallel to XY at height `z`.
    fn datum_at(z: f64) -> (&'static str, FeatureOutput) {
        let placement = partsmith_sketch::OriginPlane::Xy.placement().offset(z);
        (
            "f-dp1",
            FeatureOutput {
                data: OutputData::Datum(DatumResult::Plane(placement)),
                warnings: Vec::new(),
            },
        )
    }

    /// A 20 mm cube in `b-1` spanning [-10,-10,0]-[10,10,20].
    fn cube() -> Vec<(&'static str, BodyState)> {
        vec![("b-1", box_body("f-box1", [-10.0, -10.0, 0.0], 20.0))]
    }

    fn run(params: &SplitBodyParams, z: f64) -> Result<FeatureOutput, FeatureError> {
        build::<SplitBody>("f-split1", params, cube(), vec![datum_at(z)])
    }

    /// The geometry of the face named `name` in the update of `body`.
    fn face(output: &FeatureOutput, body: &str, name: &str) -> partsmith_occt_sys::ElementGeometry {
        let element = ElementName::parse(name).expect("valid name");
        let index = update(output, body)
            .naming
            .index(&element)
            .unwrap_or_else(|| panic!("{body} has no face {name}"));
        updated_shape(output, body)
            .element(SubShapeKind::Face, index)
            .expect("the face has geometry")
    }

    #[test]
    fn canonical_params_omit_flip_and_a_missing_target() {
        let params = split_body(datum(), json!({"flip": false}));

        assert_eq!(
            serde_json::to_string(&params).expect("serializes"),
            r#"{"body":"b-1","plane":{"kind":"datum","feature":"f-dp1"}}"#
        );
    }

    #[test]
    fn the_target_is_not_the_body() {
        let error = SplitBody::validate(&split_body(datum(), json!({"target": "b-1"})))
            .expect_err("b-1 cannot receive its own other part");

        assert_eq!(error.code(), "same_body");
        assert_eq!(error.field().expect("names a field").path, "target");
    }

    #[test]
    fn dependencies_write_the_body_then_the_target() {
        let dependencies = SplitBody::dependencies(&split_body(datum(), json!({"target": "b-2"})));

        let written: Vec<(&str, &str)> = dependencies
            .bodies_written
            .iter()
            .map(|link| (link.id.as_str(), link.field.as_str()))
            .collect();
        assert_eq!(written, [("b-1", "body"), ("b-2", "target")]);
        assert_eq!(dependencies.features[0].id, id("f-dp1"));
    }

    #[test]
    fn the_body_keeps_the_side_the_normal_points_to() {
        let output =
            run(&split_body(datum(), json!({"target": "b-2"})), 5.0).expect("the split builds");

        let upper = updated_shape(&output, "b-1");
        assert_volume(upper, 6000.0);
        assert_bounds(upper, [-10.0, -10.0, 5.0], [10.0, 10.0, 20.0]);
        let lower = updated_shape(&output, "b-2");
        assert_volume(lower, 2000.0);
        let changes: Vec<BodyChange> = updates(&output)
            .iter()
            .map(|update| update.change)
            .collect();
        assert_eq!(changes, [BodyChange::Modify, BodyChange::New]);
    }

    #[test]
    fn section_faces_are_named_by_side() {
        let output =
            run(&split_body(datum(), json!({"target": "b-2"})), 5.0).expect("the split builds");

        let upper = face(&output, "b-1", "f-split1/split-positive");
        assert_eq!(upper.kind, GeometryKind::Plane);
        let normal = upper.direction.expect("a plane has a normal");
        assert!(
            (normal.z + 1.0).abs() < 1e-9,
            "outward normal is -Z, got {normal:?}"
        );
        let lower = face(&output, "b-2", "f-split1/split-negative");
        assert!((lower.centroid.z - 5.0).abs() < 1e-9);
    }

    #[test]
    fn pieces_of_a_cut_face_are_numbered_across_both_parts() {
        let output =
            run(&split_body(datum(), json!({"target": "b-2"})), 5.0).expect("the split builds");

        assert!((face(&output, "b-1", "f-box1/front#2").centroid.z - 12.5).abs() < 1e-9);
        assert!((face(&output, "b-2", "f-box1/front#1").centroid.z - 2.5).abs() < 1e-9);
        assert!(face_names(&update(&output, "b-1").naming).contains(&"f-box1/top".to_string()));
        assert!(face_names(&update(&output, "b-2").naming).contains(&"f-box1/bottom".to_string()));
    }

    #[test]
    fn a_trim_discards_the_other_part() {
        let output = run(&split_body(datum(), json!({})), 5.0).expect("the trim builds");

        assert_eq!(updates(&output).len(), 1);
        assert_volume(updated_shape(&output, "b-1"), 6000.0);
    }

    #[test]
    fn flip_keeps_the_back_part() {
        let output = run(
            &split_body(datum(), json!({"flip": true, "target": "b-2"})),
            5.0,
        )
        .expect("the split builds");

        assert_volume(updated_shape(&output, "b-1"), 2000.0);
        assert!(
            face_names(&update(&output, "b-1").naming)
                .contains(&"f-split1/split-negative".to_string())
        );
        assert_volume(updated_shape(&output, "b-2"), 6000.0);
    }

    #[test]
    fn a_plane_that_misses_the_body_fails() {
        let error = run(&split_body(datum(), json!({"target": "b-2"})), 30.0)
            .expect_err("the plane is above the body");

        assert_eq!(error, FeatureError::PlaneMissesBody { body: id("b-1") });
    }

    #[test]
    fn a_plane_on_a_face_only_touches_the_body() {
        let error = run(&split_body(datum(), json!({"target": "b-2"})), 0.0)
            .expect_err("the plane lies on the bottom face");

        assert_eq!(error.code(), "plane_misses_body");
    }

    #[test]
    fn a_target_with_a_solid_cannot_receive_the_other_part() {
        let mut bodies = cube();
        bodies.push(("b-2", box_body("f-box2", [50.0, 0.0, 0.0], 1.0)));

        let error = build::<SplitBody>(
            "f-split1",
            &split_body(datum(), json!({"target": "b-2"})),
            bodies,
            vec![datum_at(5.0)],
        )
        .expect_err("b-2 holds a solid");

        assert_eq!(error, FeatureError::BodyNotEmpty { body: id("b-2") });
    }

    #[test]
    fn python_writes_the_plane_into_and_flip() {
        assert_eq!(
            python::<SplitBody>(&split_body(datum(), json!({"target": "b-2"}))).as_deref(),
            Some("split(plane_1, into=part_2)")
        );
        assert_eq!(
            python::<SplitBody>(&split_body(
                json!({"kind": "origin", "plane": "XZ"}),
                json!({"flip": true})
            ))
            .as_deref(),
            Some("split(Plane.XZ, flip=True)")
        );
    }
}
