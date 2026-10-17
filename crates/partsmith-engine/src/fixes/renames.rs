//! The face names `TurnDrawingBack` changes (engine.md section 3.22): a feature names faces after
//! the sketch points it uses (a hole's `<id>/bore/<point>`, features/hole.md section 3.4), so when
//! the mirrored drawing swaps the ends of an arc, the faces made from them swap names, and the
//! references later features store to them are renamed in the same batch.

use std::collections::BTreeMap;

use partsmith_model::Id;
use partsmith_naming::{
    Anchors, ElementKind, ElementName, FaceGenerator, Merged, MergedName, NamingError, Reference,
};

use crate::error::EngineError;

/// For each feature, the sketch points whose names it changes: the point as it was written to the
/// point as it is written now.
#[derive(Debug, Default)]
pub(super) struct PointRenames(BTreeMap<Id, BTreeMap<String, String>>);

impl PointRenames {
    /// Records that `feature` now writes the point `from` as `to`.
    pub(super) fn add(&mut self, feature: &Id, from: String, to: String) {
        self.0.entry(feature.clone()).or_default().insert(from, to);
    }

    /// `reference` with every name it holds renamed (its query, the parts and faces of its
    /// anchors, and its merged names); `None` when it names no renamed face.
    pub(super) fn reference(
        &self,
        reference: &Reference,
    ) -> Result<Option<Reference>, EngineError> {
        let anchors = reference
            .anchors
            .0
            .iter()
            .map(|(part, faces)| Ok((self.name(part)?, self.names(faces)?)))
            .collect::<Result<_, EngineError>>()?;
        let merged = reference
            .merged
            .0
            .iter()
            .map(|(face, former)| Ok((self.name(face)?, self.merged_names(former)?)))
            .collect::<Result<_, EngineError>>()?;
        let renamed = Reference {
            query: self.name(&reference.query)?,
            fingerprint: reference.fingerprint.clone(),
            anchors: Anchors(anchors),
            merged: Merged(merged),
        };
        Ok((renamed != *reference).then_some(renamed))
    }

    /// `name` with the point generator of every face of a renamed feature in it renamed, at any
    /// depth, and the face terms of its edge and vertex names sorted again.
    fn name(&self, name: &ElementName) -> Result<ElementName, EngineError> {
        // Most names name no renamed feature at all.
        if !self
            .0
            .keys()
            .any(|feature| name.as_str().contains(feature.as_str()))
        {
            return Ok(name.clone());
        }
        match name.kind() {
            ElementKind::Face => self.face(name),
            ElementKind::Edge => self.by_face_terms(name, ElementName::edge),
            ElementKind::Vertex => self.by_face_terms(name, ElementName::vertex),
        }
    }

    /// A face name renamed (`name`): `<feature>/<role>[/<generator>]` and its own groups.
    fn face(&self, name: &ElementName) -> Result<ElementName, EngineError> {
        let parts = name
            .face_name()
            .ok_or_else(|| EngineError::internal("a face name reads as one"))?;
        let generator = match parts.generator() {
            None => return Ok(name.clone()),
            Some(FaceGenerator::Sketch(point)) => self
                .0
                .get(parts.feature())
                .and_then(|points| points.get(point))
                .map_or(point, String::as_str)
                .to_owned(),
            Some(FaceGenerator::Face(inner) | FaceGenerator::List(inner)) => {
                self.name(&parsed(inner)?)?.as_str().to_owned()
            }
        };
        parsed(&format!(
            "{}/{}/{generator}{}",
            parts.feature(),
            parts.role(),
            parts.groups()
        ))
    }

    /// An edge or vertex name renamed (`name`): its face terms renamed and put together again by
    /// `join` (`ElementName::edge` or `ElementName::vertex`, which sort them), then its own `#n`
    /// groups.
    fn by_face_terms(
        &self,
        name: &ElementName,
        join: impl FnOnce(&[ElementName]) -> Result<ElementName, NamingError>,
    ) -> Result<ElementName, EngineError> {
        let base = name.split_base();
        let pieces = &name.as_str()[base.as_str().len()..];
        let faces = base
            .face_terms()
            .iter()
            .map(|face| self.name(face))
            .collect::<Result<Vec<_>, _>>()?;
        let renamed = join(&faces).map_err(EngineError::unexpected)?;
        parsed(&format!("{renamed}{pieces}"))
    }

    /// Names renamed, sorted and distinct, as anchors list them.
    fn names(&self, names: &[ElementName]) -> Result<Vec<ElementName>, EngineError> {
        renamed_sorted(names, |name| self.name(name))
    }

    /// Merged names renamed, sorted and distinct, as references list them.
    fn merged_names(&self, former: &[MergedName]) -> Result<Vec<MergedName>, EngineError> {
        renamed_sorted(former, |merged| {
            Ok(match merged {
                MergedName::Face(face) => MergedName::Face(self.name(face)?),
                MergedName::Copies { source, feature } => MergedName::Copies {
                    source: self.name(source)?,
                    feature: feature.clone(),
                },
            })
        })
    }
}

/// `items` each renamed by `rename`, sorted and distinct.
fn renamed_sorted<T: Ord>(
    items: &[T],
    rename: impl Fn(&T) -> Result<T, EngineError>,
) -> Result<Vec<T>, EngineError> {
    let mut renamed = items.iter().map(rename).collect::<Result<Vec<_>, _>>()?;
    renamed.sort_unstable();
    renamed.dedup();
    Ok(renamed)
}

/// The element name `text`, which a valid name gave apart or a rename put together.
fn parsed(text: &str) -> Result<ElementName, EngineError> {
    ElementName::parse(text).map_err(EngineError::unexpected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// The renames of a hole `f-hole1` whose points `e-5.start` and `e-5.end` swapped.
    fn swapped_ends() -> PointRenames {
        let mut renames = PointRenames::default();
        renames.add(&id("f-hole1"), "e-5.start".into(), "e-5.end".into());
        renames.add(&id("f-hole1"), "e-5.end".into(), "e-5.start".into());
        renames
    }

    /// The valid name `text`.
    fn name(text: &str) -> ElementName {
        ElementName::parse(text).expect("a valid name")
    }

    /// A face or edge reference with a fingerprint of its kind.
    fn reference(query: &str) -> Reference {
        let fingerprint = if name(query).kind() == ElementKind::Face {
            "plane|n=0,0,1"
        } else {
            "circle|n=0,0,1"
        };
        Reference::new(
            name(query),
            fingerprint.parse().expect("a valid fingerprint"),
        )
        .expect("a reference")
    }

    #[test]
    fn the_faces_a_feature_made_from_a_swapped_point_swap_names() {
        let renames = swapped_ends();

        assert_eq!(
            renames
                .name(&name("f-hole1/bore/e-5.start"))
                .expect("renames"),
            name("f-hole1/bore/e-5.end")
        );
        assert_eq!(
            renames
                .name(&name("f-hole1/entry/e-5.end#2"))
                .expect("renames"),
            name("f-hole1/entry/e-5.start#2")
        );
        assert_eq!(
            renames.name(&name("f-hole1/bore/e-6")).expect("renames"),
            name("f-hole1/bore/e-6")
        );
    }

    #[test]
    fn faces_of_other_features_keep_their_names() {
        let renames = swapped_ends();

        for unchanged in [
            "f-hole2/bore/e-5.start",
            "f-box1/top",
            "f-hole10/bore/e-5.start",
        ] {
            assert_eq!(
                renames.name(&name(unchanged)).expect("renames"),
                name(unchanged)
            );
        }
    }

    #[test]
    fn edge_names_sort_their_renamed_faces_again() {
        let renames = swapped_ends();

        let renamed = renames
            .name(&name(
                "edge(f-hole1/bore/e-5.start|f-hole1/bottom/e-5.start)#2",
            ))
            .expect("renames");

        assert_eq!(
            renamed,
            name("edge(f-hole1/bore/e-5.end|f-hole1/bottom/e-5.end)#2")
        );
        assert_eq!(
            renames
                .name(&name("edge(f-hole1/bore/e-5.end|f-hole1/bore/e-5.start)"))
                .expect("renames"),
            name("edge(f-hole1/bore/e-5.end|f-hole1/bore/e-5.start)")
        );
    }

    #[test]
    fn names_nested_in_generators_are_renamed() {
        let renames = swapped_ends();

        let renamed = renames
            .name(&name(
                "edge(f-box1/top|f-fil1/fillet/edge(f-box1/top|f-hole1/bore/e-5.start)@f-lp1:2)",
            ))
            .expect("renames");

        assert_eq!(
            renamed,
            name("edge(f-box1/top|f-fil1/fillet/edge(f-box1/top|f-hole1/bore/e-5.end)@f-lp1:2)")
        );
        assert_eq!(
            renames
                .name(&name("f-sh1/rim/f-hole1/bore/e-5.end@f-lp1:1"))
                .expect("renames"),
            name("f-sh1/rim/f-hole1/bore/e-5.start@f-lp1:1")
        );
    }

    #[test]
    fn a_reference_renames_its_query_anchors_and_merged_names() {
        let renames = swapped_ends();
        let mut stored = reference("edge(f-box1/top#2|f-hole1/bore/e-5.start)");
        stored.anchors = Anchors(BTreeMap::from([(
            name("f-box1/top#2"),
            vec![name("f-hole1/bore/e-5.start"), name("f-hole1/bore/e-6")],
        )]));
        stored.merged = Merged(BTreeMap::from([(
            name("f-hole1/bore/e-5.start"),
            vec![MergedName::Face(name("f-hole1/bore/e-5.end"))],
        )]));

        let renamed = renames
            .reference(&stored)
            .expect("renames")
            .expect("the reference names a renamed face");

        assert_eq!(
            renamed.query,
            name("edge(f-box1/top#2|f-hole1/bore/e-5.end)")
        );
        assert_eq!(renamed.fingerprint, stored.fingerprint);
        assert_eq!(
            renamed.anchors.0[&name("f-box1/top#2")],
            [name("f-hole1/bore/e-5.end"), name("f-hole1/bore/e-6")]
        );
        assert_eq!(
            renamed.merged.0[&name("f-hole1/bore/e-5.end")],
            [MergedName::Face(name("f-hole1/bore/e-5.start"))]
        );
    }

    #[test]
    fn a_reference_that_names_no_renamed_face_is_not_rewritten() {
        let renames = swapped_ends();

        assert_eq!(
            renames
                .reference(&reference("edge(f-box1/front|f-box1/top)"))
                .expect("renames"),
            None
        );
    }
}
