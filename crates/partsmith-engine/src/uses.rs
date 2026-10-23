//! What every feature uses (engine.md section 3.24), from the document alone, without
//! recomputing or calling the kernel: the features it links to, and the features and sketch
//! entities that the element names in its stored references depend on.

use partsmith_features::{Link, Registry};
use partsmith_model::{Document, FeatureRecord, Id};
use partsmith_naming::{GeneratorUse, MergedName, Reference};
use partsmith_protocol::{DocHandle, Use};

use crate::error::EngineError;
use crate::params::{declared_dependencies, field_pointer, stored_references};
use crate::session::Session;

impl Session {
    /// What every feature uses, from the document alone (section 3.24): one `Use` per feature
    /// link and per feature and sketch entity an element name of a stored reference depends on,
    /// sorted by the user's timeline position, then field, feature and entity, without
    /// duplicates. Records nothing and emits nothing.
    pub fn list_uses(&self, doc: DocHandle) -> Result<Vec<Use>, EngineError> {
        Ok(uses(&self.registry, &self.open(doc)?.document))
    }
}

/// What every feature of `document` uses (section 3.24).
fn uses(registry: &Registry, document: &Document) -> Vec<Use> {
    let mut found: Vec<(usize, Use)> = Vec::new();
    for (position, record) in document.features.iter().enumerate() {
        for link in feature_links(registry, record) {
            found.push((
                position,
                Use {
                    user: record.id.clone(),
                    field: field_pointer(&link.field),
                    feature: link.id,
                    entity: None,
                },
            ));
        }
        for stored in stored_references(&record.params) {
            for generator in generators(&stored.reference) {
                found.push((
                    position,
                    Use {
                        user: record.id.clone(),
                        field: stored.pointer.clone(),
                        entity: generator.generator.as_deref().and_then(entity_of),
                        feature: generator.feature,
                    },
                ));
            }
        }
    }
    // `None` sorts before every entity, so a use without one comes first.
    found.sort_by(|(first_position, first), (second_position, second)| {
        (first_position, &first.field, &first.feature, &first.entity).cmp(&(
            second_position,
            &second.field,
            &second.feature,
            &second.entity,
        ))
    });
    found.dedup();
    found.into_iter().map(|(_, used)| used).collect()
}

/// The features `record` links to (`Dependencies::features`); none when its params fail
/// validation.
fn feature_links(registry: &Registry, record: &FeatureRecord) -> Vec<Link> {
    declared_dependencies(registry, record)
        .map(|dependencies| dependencies.features)
        .unwrap_or_default()
}

/// The feature and sketch generators the element names of `reference` depend on: those of its
/// query and of each of its merged names, a name of every copy standing for its source and the
/// pattern or mirror that made it (naming.md section 2.4).
fn generators(reference: &Reference) -> Vec<GeneratorUse> {
    let mut found = reference.query.generators();
    for merged in reference.merged.0.values().flatten() {
        match merged {
            MergedName::Face(name) => found.extend(name.generators()),
            MergedName::Copies { source, feature } => {
                found.extend(source.generators());
                found.push(GeneratorUse {
                    feature: feature.clone(),
                    generator: None,
                });
            }
        }
    }
    found
}

/// The sketch entity a generator names: the part before any `.`, so `e-1.center` gives `e-1`.
fn entity_of(generator: &str) -> Option<Id> {
    let entity = generator.split('.').next().unwrap_or(generator);
    Id::new(entity).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    /// A reference to `query` with a plane fingerprint and these merged names.
    fn reference(query: &str, merged: serde_json::Value) -> Reference {
        serde_json::from_value(serde_json::json!({
            "q": query,
            "fp": "plane|n=0,0,1|c=0,0,10|a=100|adj=4,4",
            "merged": merged
        }))
        .expect("a valid reference")
    }

    #[test]
    fn a_copy_name_stands_for_its_source_and_the_feature_that_made_it() {
        let top = reference(
            "f-box1/top",
            serde_json::json!({"f-box1/top": ["f-box1/top@f-lp1:*"]}),
        );

        let found: Vec<Id> = generators(&top)
            .into_iter()
            .map(|generator| generator.feature)
            .collect();

        assert_eq!(found, [id("f-box1"), id("f-box1"), id("f-lp1")]);
    }

    #[test]
    fn an_entity_is_named_without_its_point_role() {
        assert_eq!(entity_of("e-1.center"), Some(id("e-1")));
        assert_eq!(entity_of("e-p2"), Some(id("e-p2")));
    }
}
