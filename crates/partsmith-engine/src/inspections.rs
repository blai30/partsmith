//! The registry's inspection of each feature record, memoized for the whole session by the
//! record's content, so that one edit inspects only the records it changed and a document opened
//! again inspects none: the count of numbers without a unit and Keep sizes read it (engine.md
//! section 3.25), and so do the ids a command creates.

use std::collections::BTreeMap;

use partsmith_features::Registry;
use partsmith_model::{Document, FeatureInspector, FeatureLinks, FeatureRecord, Id};
use serde_json::{Map, Value};

/// The inspections kept besides those the latest count used: the records of about a dozen
/// documents of 300 features, as many as recompute's default result cache keeps.
const KEPT_INSPECTIONS: usize = 4096;

/// The registry's inspection of feature records, for the whole session, memoized by the content
/// it is computed from (the record's id, kind and params, as recompute memoizes validation,
/// recompute.md section 3.1), so that documents with the same records share them. Each count is
/// one pass: the inspections it used stay, and beyond `KEPT_INSPECTIONS` the ones used longest
/// ago go.
#[derive(Debug, Default)]
pub(crate) struct Inspections {
    /// The inspections of each feature id, one per kind and params.
    by_feature: BTreeMap<Id, Vec<Inspected>>,
    /// The number of inspections kept.
    len: usize,
    /// The number of the latest count.
    pass: u64,
}

/// One record's inspection, what it was computed from, and the count that last used it.
#[derive(Debug)]
struct Inspected {
    /// The record's feature type.
    kind: String,
    /// The record's params.
    params: Map<String, Value>,
    /// The registry's links of the record, or why its params are invalid.
    links: Result<FeatureLinks, String>,
    /// The count that last used it.
    last_use: u64,
}

impl Inspected {
    /// True when it was computed from the kind and params of `record`, a record with its id.
    fn is_of(&self, record: &FeatureRecord) -> bool {
        self.kind == record.kind && self.params == record.params
    }
}

impl Inspections {
    /// Counts the numbers without a unit of `document` (model.md `bare_numbers`) as one pass:
    /// inspects the records no count inspected with the same content, then counts through the
    /// memoized inspections.
    pub(crate) fn count(&mut self, registry: &Registry, document: &Document) -> usize {
        self.refresh(registry, document);
        document.bare_numbers(&self.with(registry))
    }

    /// Starts a count over `document`: inspects each of its records that no count inspected with
    /// the same content, marks the inspection of every record used, and then drops the
    /// inspections used longest ago beyond `KEPT_INSPECTIONS`.
    fn refresh(&mut self, registry: &Registry, document: &Document) {
        self.pass += 1;
        let pass = self.pass;
        for record in &document.features {
            let reused = self
                .by_feature
                .get_mut(&record.id)
                .and_then(|of_feature| of_feature.iter_mut().find(|kept| kept.is_of(record)));
            if let Some(inspected) = reused {
                inspected.last_use = pass;
                continue;
            }
            self.by_feature
                .entry(record.id.clone())
                .or_default()
                .push(Inspected {
                    kind: record.kind.clone(),
                    params: record.params.clone(),
                    links: registry.inspect(record),
                    last_use: pass,
                });
            self.len += 1;
        }
        self.evict_to(KEPT_INSPECTIONS);
    }

    /// While more than `limit` inspections are kept, drops the ones used longest ago among those
    /// the latest count did not use; inspections last used by the same count go together.
    fn evict_to(&mut self, limit: usize) {
        let excess = self.len.saturating_sub(limit);
        if excess == 0 {
            return;
        }
        let mut unused: Vec<u64> = self
            .by_feature
            .values()
            .flatten()
            .map(|inspected| inspected.last_use)
            .filter(|last_use| *last_use != self.pass)
            .collect();
        unused.sort_unstable();
        let Some(&cutoff) = unused.get(excess.min(unused.len()).saturating_sub(1)) else {
            return;
        };
        self.by_feature.retain(|_, of_feature| {
            of_feature.retain(|inspected| inspected.last_use > cutoff);
            !of_feature.is_empty()
        });
        self.len = self.by_feature.values().map(Vec::len).sum();
    }

    /// The memoized inspection of `record`, when one was computed from the same content.
    fn memoized(&self, record: &FeatureRecord) -> Option<&Result<FeatureLinks, String>> {
        self.by_feature
            .get(&record.id)?
            .iter()
            .find(|inspected| inspected.is_of(record))
            .map(|inspected| &inspected.links)
    }

    /// The registry as an inspector that reads these inspections for the records they have.
    pub(crate) fn with<'a>(&'a self, registry: &'a Registry) -> MemoizedInspector<'a> {
        MemoizedInspector {
            registry,
            inspections: self,
        }
    }

    /// The number of inspections kept.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The feature ids with an inspection, one per inspection.
    #[cfg(test)]
    pub(crate) fn inspected(&self) -> Vec<String> {
        self.by_feature
            .iter()
            .flat_map(|(feature, inspected)| inspected.iter().map(|_| feature.to_string()))
            .collect()
    }
}

/// The registry as a `FeatureInspector` that reuses the memoized inspection of a record with the
/// same content, and asks the registry for every other record.
pub(crate) struct MemoizedInspector<'a> {
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
    fn a_document_opened_again_reuses_every_inspection() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        let records = vec![box_record("f-box1", "10"), box_record("f-box2", "20")];
        inspections.refresh(&registry, &document(records.clone()));

        inspections.refresh(&registry, &document(records));

        assert_eq!(inspections.inspected(), ["f-box1", "f-box2"]);
        assert_eq!(inspections.len, 2);
    }

    #[test]
    fn records_with_the_same_id_and_another_content_each_keep_their_inspection() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        let ten = box_record("f-box1", "10");
        let twenty = box_record("f-box1", "20 mm");

        inspections.refresh(&registry, &document(vec![ten.clone()]));
        inspections.refresh(&registry, &document(vec![twenty.clone()]));

        assert_eq!(inspections.inspected(), ["f-box1", "f-box1"]);
        assert!(inspections.memoized(&ten).is_some());
        assert!(inspections.memoized(&twenty).is_some());
    }

    #[test]
    fn eviction_drops_the_inspections_used_longest_ago_and_keeps_the_latest_count_s() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        inspections.refresh(&registry, &document(vec![box_record("f-box1", "10")]));
        inspections.refresh(&registry, &document(vec![box_record("f-box2", "10")]));
        inspections.refresh(&registry, &document(vec![box_record("f-box3", "10")]));

        inspections.evict_to(2);
        let after_one = inspections.inspected();
        inspections.evict_to(0);

        assert_eq!(after_one, ["f-box2", "f-box3"]);
        assert_eq!(inspections.inspected(), ["f-box3"]);
        assert_eq!(inspections.len, 1);
    }

    #[test]
    fn eviction_keeps_everything_within_the_limit() {
        let registry = Registry::with_builtins();
        let mut inspections = Inspections::default();
        inspections.refresh(&registry, &document(vec![box_record("f-box1", "10")]));
        inspections.refresh(&registry, &document(vec![box_record("f-box2", "10")]));

        inspections.evict_to(2);

        assert_eq!(inspections.inspected(), ["f-box1", "f-box2"]);
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
