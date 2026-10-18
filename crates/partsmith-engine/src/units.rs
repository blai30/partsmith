//! Document units (engine.md section 3.25): `set_document_units` with Keep sizes, and the count
//! of numbers without a unit that `document_changed` reports, both through the registry's
//! inspection of each feature record, memoized by the record's content so that one edit inspects
//! only the records it changed.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_model::{Document, FeatureInspector, FeatureLinks, FeatureRecord, Id, Units};
use partsmith_protocol::DocHandle;
use serde_json::{Map, Value};

use crate::counts::saturating_u32;
use crate::document::OpenDocument;
use crate::error::EngineError;
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
        let registry = Arc::clone(&self.registry);
        let open = self.open_mut(doc)?;
        if units == open.document.units {
            return Ok(UnitsChanged {
                revision: open.revision,
                kept: 0,
            });
        }
        let kept = if keep_sizes {
            open.bare_numbers(&registry)
        } else {
            0
        };
        let edits = open
            .document
            .units_edits(units, keep_sizes, &open.inspections.with(&registry));
        let revision =
            self.apply_batch(doc, &format!("Change units to {}", units.symbol()), edits)?;
        Ok(UnitsChanged { revision, kept })
    }
}

impl OpenDocument {
    /// How many expressions and parameters of the current recipe are numbers without a unit whose
    /// meaning the document units decide (model.md `bare_numbers`), counted once per revision.
    pub(crate) fn bare_numbers(&mut self, registry: &Registry) -> u32 {
        if let Some((revision, count)) = self.bare_count
            && revision == self.revision
        {
            return count;
        }
        self.inspections.refresh(registry, &self.document);
        let count = saturating_u32(self.document.bare_numbers(&self.inspections.with(registry)));
        self.bare_count = Some((self.revision, count));
        count
    }
}

/// The registry's inspection of each feature record of one document, by feature, with the kind
/// and params it was computed from. Inspection reads nothing else of the record, so a record with
/// the same kind and params reuses it.
#[derive(Debug, Default)]
pub(crate) struct Inspections {
    /// The latest inspection of each feature.
    by_feature: BTreeMap<Id, Inspected>,
}

/// One record's inspection and what it was computed from.
#[derive(Debug)]
struct Inspected {
    /// The record's feature type.
    kind: String,
    /// The record's params.
    params: Map<String, Value>,
    /// The registry's links of the record, or why its params are invalid.
    links: Result<FeatureLinks, String>,
}

impl Inspections {
    /// Inspects every record of `document` that changed since its last inspection, and forgets
    /// the features the document no longer has.
    fn refresh(&mut self, registry: &Registry, document: &Document) {
        self.by_feature
            .retain(|feature, _| document.feature(feature).is_some());
        for record in &document.features {
            if self.memoized(record).is_none() {
                self.by_feature.insert(
                    record.id.clone(),
                    Inspected {
                        kind: record.kind.clone(),
                        params: record.params.clone(),
                        links: registry.inspect(record),
                    },
                );
            }
        }
    }

    /// The memoized inspection of `record`, when one was computed from the same kind and params.
    fn memoized(&self, record: &FeatureRecord) -> Option<&Result<FeatureLinks, String>> {
        self.by_feature
            .get(&record.id)
            .filter(|inspected| inspected.kind == record.kind && inspected.params == record.params)
            .map(|inspected| &inspected.links)
    }

    /// The registry as an inspector that reads these inspections for unchanged records.
    fn with<'a>(&'a self, registry: &'a Registry) -> MemoizedInspector<'a> {
        MemoizedInspector {
            registry,
            inspections: self,
        }
    }
}

/// The registry as a `FeatureInspector` that reuses the memoized inspection of a record with the
/// same kind and params, and asks the registry for every other record.
struct MemoizedInspector<'a> {
    /// The registry.
    registry: &'a Registry,
    /// The memoized inspections.
    inspections: &'a Inspections,
}

impl FeatureInspector for MemoizedInspector<'_> {
    fn inspect(&self, feature: &FeatureRecord) -> Result<FeatureLinks, String> {
        match self.inspections.memoized(feature) {
            Some(links) => links.clone(),
            None => self.registry.inspect(feature),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A box record `feature` of the size `x` by 10 by 10, all without a unit.
    fn box_record(feature: &str, x: &str) -> FeatureRecord {
        serde_json::from_value(json!({
            "id": feature, "type": "box", "body": "b-1", "size": [x, "10", "10"], "op": "new"
        }))
        .expect("a well-formed record")
    }

    /// A millimeter document with one body and these features.
    fn document(features: Vec<FeatureRecord>) -> Document {
        let mut document: Document = serde_json::from_value(json!({
            "format": "partsmith", "format_version": 2, "units": "mm",
            "bodies": [{"id": "b-1", "name": "Body 1"}]
        }))
        .expect("a well-formed document");
        document.features = features;
        document
    }

    #[test]
    fn unchanged_records_reuse_their_inspection() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        let first = document(vec![box_record("f-box1", "10"), box_record("f-box2", "20")]);
        inspections.refresh(&registry, &first);
        let changed = document(vec![
            box_record("f-box1", "10"),
            box_record("f-box2", "30 mm"),
        ]);

        let reused = inspections.memoized(&changed.features[0]).is_some();
        let stale = inspections.memoized(&changed.features[1]).is_none();

        assert!(reused, "the unchanged box keeps its inspection");
        assert!(stale, "the edited box is inspected again");
        // Three numbers of the first box and two of the second.
        assert_eq!(changed.bare_numbers(&inspections.with(&registry)), 5);
    }

    #[test]
    fn features_that_leave_the_document_are_forgotten() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        inspections.refresh(
            &registry,
            &document(vec![box_record("f-box1", "10"), box_record("f-box2", "20")]),
        );

        inspections.refresh(&registry, &document(vec![box_record("f-box2", "20")]));

        assert_eq!(
            inspections.by_feature.keys().collect::<Vec<_>>(),
            [&Id::new("f-box2").expect("a valid id")]
        );
    }

    #[test]
    fn invalid_records_are_memoized_as_invalid() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        let mut invalid = box_record("f-box1", "10");
        invalid.params.remove("body");
        let with_invalid = document(vec![invalid.clone()]);

        inspections.refresh(&registry, &with_invalid);

        assert!(matches!(inspections.memoized(&invalid), Some(Err(_))));
        assert_eq!(with_invalid.bare_numbers(&inspections.with(&registry)), 0);
    }
}
