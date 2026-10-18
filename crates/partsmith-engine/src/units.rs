//! Document units (engine.md section 3.25): `set_document_units` with Keep sizes, and the count
//! of numbers without a unit that `document_changed` reports, both through the session's
//! memoized inspections (`Inspections`).

use partsmith_features::Registry;
use partsmith_model::{Document, Units};
use partsmith_protocol::DocHandle;

use crate::counts::saturating_u32;
use crate::document::OpenDocument;
use crate::error::EngineError;
use crate::inspections::Inspections;
use crate::session::Session;
use crate::types::UnitsChanged;

impl Session {
    /// Changes the document units as one batch, labeled `Change units to {units}` outside a
    /// transaction; with `keep_sizes`, every number without a unit whose meaning the units decide
    /// gets the old unit, so no size changes. The current units change nothing.
    pub fn set_document_units(
        &mut self,
        doc: DocHandle,
        units: Units,
        keep_sizes: bool,
    ) -> Result<UnitsChanged, EngineError> {
        let Session {
            documents,
            registry,
            inspections,
            ..
        } = self;
        let open = documents
            .get_mut(&doc)
            .ok_or(EngineError::UnknownDocument { doc })?;
        if units == open.document.units {
            return Ok(UnitsChanged {
                revision: open.revision,
                kept: 0,
            });
        }
        let kept = if keep_sizes {
            open.bare_numbers(registry, inspections)
        } else {
            0
        };
        let edits = open
            .document
            .units_edits(units, keep_sizes, &inspections.with(registry));
        let revision =
            self.apply_batch(doc, &format!("Change units to {}", units.symbol()), edits)?;
        Ok(UnitsChanged { revision, kept })
    }

    /// Counts the numbers without a unit of `document` (model.md `bare_numbers`) through the
    /// session's memoized inspections, as `document_changed` counts them after every change of a
    /// recipe: only the records no count inspected with the same content are inspected. Hosts read
    /// the count from `document_changed`; this exists so the budgets bench can time the count on
    /// its own (engine.md section 5), which `Session::call` cannot.
    #[doc(hidden)]
    pub fn count_bare_numbers(&mut self, document: &Document) -> usize {
        let Session {
            registry,
            inspections,
            ..
        } = self;
        inspections.count(registry, document)
    }
}

impl OpenDocument {
    /// How many expressions and parameters of the current recipe are numbers without a unit whose
    /// meaning the document units decide (model.md `bare_numbers`), counted once per revision
    /// through the session's `inspections`.
    pub(crate) fn bare_numbers(
        &mut self,
        registry: &Registry,
        inspections: &mut Inspections,
    ) -> u32 {
        if let Some((revision, count)) = self.bare_count
            && revision == self.revision
        {
            return count;
        }
        let count = saturating_u32(inspections.count(registry, &self.document));
        self.bare_count = Some((self.revision, count));
        count
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::box_session;

    #[test]
    fn counting_a_document_inspects_only_the_records_the_session_has_not_seen() {
        let (mut session, doc) = box_session(false);
        let mut edited = session.document(doc).expect("box.ptsm is open").clone();
        let opened = session.count_bare_numbers(&edited);
        let kept = session.inspections.len();
        edited.features[0]
            .params
            .insert("size".to_owned(), json!(["20", "30", "height"]));

        let counted = session.count_bare_numbers(&edited);

        assert_eq!((opened, counted), (0, 2));
        assert_eq!(counted, edited.bare_numbers(session.registry.as_ref()));
        assert_eq!(
            session.inspections.len(),
            kept + 1,
            "only the edited box is inspected"
        );
    }

    #[test]
    fn a_session_shares_the_inspections_of_a_document_closed_and_opened_again() {
        let (mut session, doc) = box_session(false);
        let text = session.save_document(doc).expect("box.ptsm saves").text;
        let Session {
            documents,
            registry,
            inspections,
            ..
        } = &mut session;
        let first = documents
            .get_mut(&doc)
            .map(|open| open.bare_numbers(registry, inspections));
        let kept = inspections.len();
        session.close_document(doc).expect("the document closes");

        let reopened = session
            .open_document(&text, Vec::new())
            .expect("the text opens")
            .doc;
        let Session {
            documents,
            registry,
            inspections,
            ..
        } = &mut session;
        let second = documents
            .get_mut(&reopened)
            .map(|open| open.bare_numbers(registry, inspections));

        assert_eq!(first, second);
        assert_eq!(inspections.len(), kept, "no record was inspected again");
        assert_eq!(inspections.inspected(), ["f-box1"]);
    }
}
