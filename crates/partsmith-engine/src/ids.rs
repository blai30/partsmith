//! Ids for the records a command creates: a fix's body or constraint (engine.md section 3.22) and
//! an import's body and feature (section 3.14).

use std::collections::BTreeSet;

use partsmith_model::{Document, Id, IdGenerator, IdKind};

use crate::error::EngineError;
use crate::units::MemoizedInspector;

/// Ids for the records one command creates, from the session's generator, so that
/// `SequentialIds` makes them deterministic, and never an id the document uses or one made
/// before.
pub(crate) struct NewIds<'a> {
    /// The session's generator.
    generator: &'a mut (dyn IdGenerator + Send),
    /// The document the command changes.
    document: &'a Document,
    /// The registry's inspections, which read the ids feature params declare.
    inspector: MemoizedInspector<'a>,
    /// The ids in use, found when the first id is made, with every id made since.
    used: Option<BTreeSet<Id>>,
}

impl<'a> NewIds<'a> {
    /// Ids from `generator` that `document` does not use, with the ids its feature params declare
    /// read through `inspector`.
    pub(crate) fn new(
        generator: &'a mut (dyn IdGenerator + Send),
        document: &'a Document,
        inspector: MemoizedInspector<'a>,
    ) -> NewIds<'a> {
        NewIds {
            generator,
            document,
            inspector,
            used: None,
        }
    }

    /// A new id of `kind`, unused by the document and by the ids made before it.
    pub(crate) fn next(&mut self, kind: IdKind) -> Result<Id, EngineError> {
        let used = self
            .used
            .get_or_insert_with(|| self.document.used_ids(&self.inspector));
        let id = self
            .generator
            .generate(kind, &|id: &Id| used.contains(id))
            .map_err(EngineError::unexpected)?;
        used.insert(id.clone());
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use partsmith_features::Registry;
    use partsmith_model::{IdError, SequentialIds};
    use serde_json::json;

    use super::*;
    use crate::test_support::id;
    use crate::units::Inspections;

    /// A generator that hands out the first id of a kind that is not taken.
    struct FirstFree;

    impl IdGenerator for FirstFree {
        fn generate(
            &mut self,
            kind: IdKind,
            is_taken: &dyn Fn(&Id) -> bool,
        ) -> Result<Id, IdError> {
            (1..)
                .map(|value| Id::generated(kind, value))
                .find(|candidate| !is_taken(candidate))
                .ok_or(IdError::Exhausted { attempts: 0 })
        }
    }

    /// A document with the body `b-000000000001` and a sketch whose constraint is
    /// `c-000000000001`, the first ids `SequentialIds` makes of their kinds.
    fn document_using_the_first_ids() -> Document {
        serde_json::from_value(json!({
            "format": "partsmith", "format_version": 2, "units": "mm",
            "bodies": [{"id": "b-000000000001", "name": "Body 1"}],
            "features": [{
                "id": "f-sk1", "type": "sketch", "plane": {"kind": "origin", "plane": "XY"},
                "entities": [
                    {"id": "e-1", "type": "line", "start": [0, 0], "end": [10, 0]},
                    {"id": "e-2", "type": "line", "start": [10, 0], "end": [10, 10]}
                ],
                "constraints": [
                    {"id": "c-000000000001", "type": "coincident", "points": ["e-1.end", "e-2.start"]}
                ]
            }]
        }))
        .expect("a well-formed document")
    }

    #[test]
    fn new_ids_skip_the_ids_the_document_and_its_feature_params_use() {
        let registry = Registry::with_builtins();
        let inspections = Inspections::default();
        let document = document_using_the_first_ids();
        let mut generator = SequentialIds::new();
        let mut ids = NewIds::new(&mut generator, &document, inspections.with(&registry));

        let body = ids.next(IdKind::Body).expect("a body id");
        let constraint = ids.next(IdKind::SketchConstraint).expect("a constraint id");

        assert_eq!(body, id("b-000000000002"));
        assert_eq!(constraint, id("c-000000000002"));
    }

    #[test]
    fn new_ids_never_repeat_one_made_before() {
        let registry = Registry::with_builtins();
        let inspections = Inspections::default();
        let document = document_using_the_first_ids();
        let mut generator = FirstFree;
        let mut ids = NewIds::new(&mut generator, &document, inspections.with(&registry));

        let first = ids.next(IdKind::Body).expect("an id");
        let second = ids.next(IdKind::Body).expect("an id");

        assert_eq!(first, id("b-000000000002"));
        assert_eq!(second, id("b-000000000003"));
    }
}
