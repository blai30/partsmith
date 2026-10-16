//! The edits an offered fix builds from the current records when it is applied (engine.md
//! section 3.22), with the re-anchoring of `KeepMatch` and `KeepRegions`.

use std::collections::BTreeSet;

use partsmith_features::{FeatureWarning, Fix, Names, Place};
use partsmith_model::{Edit, FeatureRecord, Id};
use partsmith_naming::{ElementName, Generator, Reference, Resolution, resolve, roles};
use partsmith_recompute::Status;
use partsmith_sketch::{MatchMethod, RegionMatch, RegionRef, match_region};
use serde_json::Value;

use super::params::{
    is_list_entry, parent, remove, replace, replace_each, set, stored_references,
};
use crate::diagnostics::{FeatureView, RunView};
use crate::error::EngineError;

impl FeatureView<'_> {
    /// The edits of an offered `apply` fix of this feature's message, built from the current
    /// records. `KeepRegions` resolves the references of later features in `later`, a run of
    /// the same recipe in which every later feature ran.
    pub(super) fn edits(&self, fix: &Fix, later: &RunView<'_>) -> Result<Vec<Edit>, EngineError> {
        match fix {
            Fix::SetField {
                feature,
                field,
                value,
            } => {
                let record = match feature {
                    Some(feature) => self.run.document.feature(feature).ok_or_else(|| {
                        EngineError::internal("a fix sets a field of a feature that is gone")
                    })?,
                    None => self.record,
                };
                let pointer = field.pointer();
                let changed = |params: &mut Value| {
                    if value.is_null() {
                        remove(params, &pointer)
                    } else {
                        set(params, &pointer, value.clone())
                    }
                };
                Ok(vec![updated(record, changed)?])
            }
            Fix::UseReferences {
                pointer,
                references,
            } => self.replaced(pointer, reference_entries(references)?),
            Fix::KeepMatch { pointer } => {
                let elements = stored_at(self.record, pointer, self.matched_elements(pointer)?);
                self.replaced(pointer, reference_entries(elements)?)
            }
            Fix::RemoveEntry { pointer } => Ok(vec![updated(self.record, |params| {
                remove(params, pointer)
            })?]),
            Fix::UseRegions { pointer, regions } => {
                self.replaced(pointer, region_entries(regions)?)
            }
            Fix::KeepRegions { pointer } => self.kept_regions(pointer, later),
            Fix::UseBody { field, body } => Ok(vec![updated(self.record, |params| {
                set(params, &field.pointer(), Value::String(body.to_string()))
            })?]),
            Fix::Unsuppress { feature } => self.suppression(feature, false),
            Fix::Suppress { feature } => self.suppression(feature, true),
            Fix::Move { feature, place } => self.moved(feature, place),
            Fix::Pick { .. }
            | Fix::Show { .. }
            | Fix::NewBody
            | Fix::Attach { .. }
            | Fix::TurnDrawingBack { .. }
            | Fix::ConnectEnds { .. }
            | Fix::DeleteConstraint { .. }
            | Fix::MakeReference { .. }
            | Fix::RenameKey { .. } => Err(EngineError::internal(
                "a fix that is no apply action of this phase was applied",
            )),
        }
    }

    /// One `update_feature` of this feature with the entry at `pointer` replaced by `values`.
    fn replaced(&self, pointer: &str, values: Vec<Value>) -> Result<Vec<Edit>, EngineError> {
        Ok(vec![updated(self.record, |params| {
            replace(params, pointer, values)
        })?])
    }

    /// The elements the reference at `pointer` resolves to now, as the feature's followed,
    /// guessed or widened warning about it stores them: `Reference::from_element` of each in the
    /// state just before the feature, in the resolver's order.
    fn matched_elements(&self, pointer: &str) -> Result<&[Reference], EngineError> {
        self.report
            .warnings
            .iter()
            .find_map(|warning| match warning {
                FeatureWarning::ReferenceFollowed {
                    field, elements, ..
                }
                | FeatureWarning::ReferenceGuessed {
                    field, elements, ..
                }
                | FeatureWarning::ReferenceWidened {
                    field, elements, ..
                } if field.pointer() == pointer => Some(elements.as_slice()),
                _ => None,
            })
            .ok_or_else(|| EngineError::internal("a kept match has no resolved elements"))
    }

    /// True when every feature after this one ran in this run or is suppressed, so that the
    /// state just before each of them is known: none comes after the run's stop point or at or
    /// after the rollback marker.
    pub(super) fn later_features_ran(&self) -> bool {
        self.run.report.features[self.position + 1..]
            .iter()
            .all(|feature| !matches!(feature.status, Status::RolledBack | Status::NotRun))
    }

    /// The re-anchoring of `KeepRegions` (section 3.22) for the region entry at `pointer`: the
    /// profile's whole region list stores the regions the profile uses now, and every reference
    /// of a later feature whose resolution in `later` went through a succession the old regions
    /// produced stores the elements it resolves to now, all in one batch.
    fn kept_regions(&self, pointer: &str, later: &RunView<'_>) -> Result<Vec<Edit>, EngineError> {
        let missing = || EngineError::internal("kept regions name no resolved profile");
        let list = parent(pointer);
        let profile = parent(list);
        let sketch: Id = self
            .record_value(&format!("{profile}/sketch"))
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .ok_or_else(missing)?;
        let stored: Vec<RegionRef> = self
            .record_value(list)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
            .ok_or_else(missing)?;
        let names = self.run.names();
        let result = names.sketch_result(&sketch).ok_or_else(missing)?;
        let entities: Vec<Id> = result
            .solved
            .entities
            .iter()
            .map(|entity| entity.id.clone())
            .collect();
        let mut used = BTreeSet::new();
        let mut successions = BTreeSet::new();
        for region in &stored {
            if let RegionMatch::Resolved {
                indices,
                method,
                successions: followed,
            } = match_region(&result.profile.regions, &entities, region)
            {
                used.extend(indices);
                if method == MatchMethod::Boundary {
                    for succession in followed {
                        successions.insert(self.lateral_face(&succession.entity)?);
                    }
                }
            }
        }
        let regions: Vec<RegionRef> = used
            .iter()
            .filter_map(|&index| result.profile.regions.get(index))
            .map(|region| region.reference())
            .collect();
        let regions = Value::Array(region_entries(&regions)?);
        let mut edits = vec![updated(self.record, |params| set(params, list, regions))?];
        if !successions.is_empty() {
            for position in self.position + 1..later.report.features.len() {
                if let Some(feature) = later.feature_at(position) {
                    edits.extend(feature.references_following(&successions)?);
                }
            }
        }
        Ok(edits)
    }

    /// The lateral face `<feature>/lateral/<entity>` this feature names after a sketch entity,
    /// whose succession keys a profile's successions produce (features.md section 3.9).
    fn lateral_face(&self, entity: &Id) -> Result<ElementName, EngineError> {
        ElementName::generated(
            self.id(),
            roles::LATERAL,
            Some(Generator::Sketch(entity.as_str())),
        )
        .map_err(EngineError::unexpected)
    }

    /// One `update_feature` of this feature when it stores references whose resolution went
    /// through one of `successions`, each storing the elements it resolves to now.
    fn references_following(
        &self,
        successions: &BTreeSet<ElementName>,
    ) -> Result<Option<Edit>, EngineError> {
        let mut rewrites = Vec::new();
        for stored in stored_references(&self.record.params) {
            let Some(body) = &stored.body else {
                continue;
            };
            if let Some(elements) = self.resolved_through(body, &stored.reference, successions)? {
                let entries = reference_entries(stored_at(self.record, &stored.pointer, &elements))?;
                rewrites.push((stored.pointer, entries));
            }
        }
        if rewrites.is_empty() {
            return Ok(None);
        }
        updated(self.record, |params| replace_each(params, rewrites)).map(Some)
    }

    /// The elements `reference` resolves to in the state of `body` just before this feature, as
    /// `Reference::from_element` stores them, in the resolver's order; `None` unless its
    /// resolution went through one of `successions`.
    fn resolved_through(
        &self,
        body: &Id,
        reference: &Reference,
        successions: &BTreeSet<ElementName>,
    ) -> Result<Option<Vec<Reference>>, EngineError> {
        let state = self.state_before(body);
        if state.is_empty() {
            return Ok(None);
        }
        let set = state.element_set().map_err(EngineError::unexpected)?;
        let Resolution::Resolved { elements, via, .. } = resolve(reference, &set) else {
            return Ok(None);
        };
        if !via.iter().any(|key| successions.contains(key)) {
            return Ok(None);
        }
        let picked = elements
            .iter()
            .filter_map(|name| set.elements().iter().find(|info| info.name == *name))
            .map(|info| Reference::from_element(info, &set))
            .collect();
        Ok(Some(picked))
    }

    /// One `update_feature` of `feature`'s suppression.
    fn suppression(&self, feature: &Id, suppressed: bool) -> Result<Vec<Edit>, EngineError> {
        let mut record = self
            .run
            .document
            .feature(feature)
            .ok_or_else(|| EngineError::internal("a fix suppresses a feature that is gone"))?
            .clone();
        record.suppressed = suppressed;
        Ok(vec![Edit::UpdateFeature { feature: record }])
    }

    /// One `move_feature` of `feature` to directly below or above the feature `place` names,
    /// found by id now.
    fn moved(&self, feature: &Id, place: &Place) -> Result<Vec<Edit>, EngineError> {
        let (other, offset) = match place {
            Place::Below(other) => (other, 1),
            Place::Above(other) => (other, 0),
        };
        let index = self
            .run
            .document
            .features
            .iter()
            .filter(|record| record.id != *feature)
            .position(|record| record.id == *other)
            .ok_or_else(|| EngineError::internal("a fix moves next to a feature that is gone"))?;
        Ok(vec![Edit::MoveFeature {
            id: feature.clone(),
            index: index + offset,
        }])
    }
}

/// One `update_feature` of `record` with its params changed by `change`, which says whether the
/// change applied.
fn updated(
    record: &FeatureRecord,
    change: impl FnOnce(&mut Value) -> bool,
) -> Result<Edit, EngineError> {
    let mut params = Value::Object(record.params.clone());
    if !change(&mut params) {
        return Err(EngineError::internal(
            "a fix names a field its feature's record does not have",
        ));
    }
    let Value::Object(params) = params else {
        return Err(EngineError::internal("feature params are an object"));
    };
    Ok(Edit::UpdateFeature {
        feature: FeatureRecord {
            params,
            ..record.clone()
        },
    })
}

/// What a fix stores at `pointer` of `record` for a reference that resolves to `elements`: every
/// element in a list entry, and in a field that holds one reference the first, in the resolver's
/// order, which is the element the feature uses (section 3.22).
fn stored_at<'e>(record: &FeatureRecord, pointer: &str, elements: &'e [Reference]) -> &'e [Reference] {
    if is_list_entry(&record.params, pointer) {
        elements
    } else {
        &elements[..elements.len().min(1)]
    }
}

/// References as the entries of a record's list stores them.
fn reference_entries(references: &[Reference]) -> Result<Vec<Value>, EngineError> {
    entries(serde_json::to_value(references))
}

/// Region references as the entries of a profile's region list stores them.
fn region_entries(regions: &[RegionRef]) -> Result<Vec<Value>, EngineError> {
    entries(serde_json::to_value(regions))
}

/// The entries of a serialized list.
fn entries(list: serde_json::Result<Value>) -> Result<Vec<Value>, EngineError> {
    match list.map_err(EngineError::unexpected)? {
        Value::Array(entries) => Ok(entries),
        _ => Err(EngineError::internal("a list serializes as a list")),
    }
}
