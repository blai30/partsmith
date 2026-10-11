//! Acceptance tests for ids and id generation (model.md section 6, "Ids", tests 1 to 16).

mod common;

use std::cell::Cell;
use std::collections::BTreeSet;

use common::id;
use partsmith_model::{Id, IdError, IdGenerator, IdKind, RandomIdGenerator, SequentialIds};

/// Predicate that treats no id as taken.
fn never_taken(_: &Id) -> bool {
    false
}

/// True when `text` is one of the `prefixes` followed by exactly 12 lowercase hex digits.
fn has_generated_form(text: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|prefix| {
        text.strip_prefix(prefix).is_some_and(|digits| {
            digits.len() == 12
                && digits
                    .chars()
                    .all(|character| matches!(character, '0'..='9' | 'a'..='f'))
        })
    })
}

/// The kinds of `IdKind::ALL`, repeated in order until there are `count` of them.
fn cycling_kinds(count: usize) -> impl Iterator<Item = IdKind> {
    IdKind::ALL.into_iter().cycle().take(count)
}

#[test]
fn t001_valid_ids_keep_their_text() {
    for text in ["f-3fa91c0e72b4", "f-sk1", "A_Z-09"] {
        let parsed = Id::new(text).expect("the id is valid");

        assert_eq!(parsed.as_str(), text);
    }
}

#[test]
fn t002_empty_id_is_rejected() {
    assert_eq!(Id::new(""), Err(IdError::Empty));
}

#[test]
fn t003_ids_are_at_most_64_characters() {
    let longest = Id::new("a".repeat(64)).expect("64 characters are allowed");

    assert_eq!(longest.as_str(), "a".repeat(64));
    assert_eq!(
        Id::new("a".repeat(65)),
        Err(IdError::TooLong { length: 65 })
    );
}

#[test]
fn t004_invalid_characters_are_reported() {
    assert_eq!(
        Id::new("f 1"),
        Err(IdError::InvalidCharacter {
            id: "f 1".to_string(),
            character: ' '
        })
    );
    assert_eq!(
        Id::new("f.1"),
        Err(IdError::InvalidCharacter {
            id: "f.1".to_string(),
            character: '.'
        })
    );
    assert_eq!(
        Id::new("é1"),
        Err(IdError::InvalidCharacter {
            id: "é1".to_string(),
            character: 'é'
        })
    );
}

#[test]
fn t005_ids_serialize_as_strings_and_reject_invalid_text() {
    let json = serde_json::to_string(&id("f-1")).expect("an id serializes");
    let error = serde_json::from_str::<Id>("\"f 1\"").expect_err("the id has a space");

    assert_eq!(json, "\"f-1\"");
    assert!(
        error.to_string().contains("The id 'f 1' contains ' '"),
        "unexpected message: {error}"
    );
}

#[test]
fn t006_ids_sort_by_their_bytes() {
    assert!(id("B") < id("a"));
    assert!(id("f-10") < id("f-9"));
}

#[test]
fn t007_generated_ids_use_the_low_48_bits() {
    assert_eq!(
        Id::generated(IdKind::Feature, 0x3fa9_1c0e_72b4),
        id("f-3fa91c0e72b4")
    );
    assert_eq!(Id::generated(IdKind::Parameter, 0), id("p-000000000000"));
    assert_eq!(
        Id::generated(IdKind::Body, 0xFFFF_0000_0000_0001),
        id("b-000000000001")
    );
}

#[test]
fn t008_kind_prefixes_in_declaration_order() {
    assert_eq!(
        IdKind::ALL.map(IdKind::prefix),
        ["p-", "b-", "f-", "e-", "c-"]
    );
}

#[test]
fn t009_sequential_ids_count_per_kind() {
    let mut generator = SequentialIds::new();

    let generated: Vec<Id> = [
        IdKind::Feature,
        IdKind::Feature,
        IdKind::Body,
        IdKind::SketchEntity,
    ]
    .into_iter()
    .map(|kind| {
        generator
            .generate(kind, &never_taken)
            .expect("the generator has ids left")
    })
    .collect();

    assert_eq!(
        generated,
        vec![
            id("f-000000000001"),
            id("f-000000000002"),
            id("b-000000000001"),
            id("e-000000000001"),
        ]
    );
}

#[test]
fn t010_sequential_ids_skip_taken_ids() {
    let taken = [id("f-000000000001"), id("f-000000000002")];
    let is_taken = |candidate: &Id| taken.contains(candidate);
    let mut generator = SequentialIds::new();

    let first = generator.generate(IdKind::Feature, &is_taken);
    let second = generator.generate(IdKind::Feature, &is_taken);

    assert_eq!(first, Ok(id("f-000000000003")));
    assert_eq!(second, Ok(id("f-000000000004")));
}

#[test]
fn t011_sequential_generators_are_deterministic() {
    let mut first = SequentialIds::new();
    let mut second = SequentialIds::new();

    let first_ids: Vec<Id> = cycling_kinds(1000)
        .map(|kind| first.generate(kind, &never_taken).expect("an id"))
        .collect();
    let second_ids: Vec<Id> = cycling_kinds(1000)
        .map(|kind| second.generate(kind, &never_taken).expect("an id"))
        .collect();

    assert_eq!(first_ids.len(), 1000);
    assert_eq!(first_ids, second_ids);
}

#[test]
fn t012_cloned_sequential_generator_continues_like_its_original() {
    let mut original = SequentialIds::new();
    for kind in [IdKind::Feature, IdKind::Body, IdKind::Feature] {
        original
            .generate(kind, &never_taken)
            .expect("the generator has ids left");
    }
    let mut copy = original.clone();

    let original_ids: Vec<Id> = cycling_kinds(10)
        .map(|kind| original.generate(kind, &never_taken).expect("an id"))
        .collect();
    let copy_ids: Vec<Id> = cycling_kinds(10)
        .map(|kind| copy.generate(kind, &never_taken).expect("an id"))
        .collect();

    assert_eq!(original_ids, copy_ids);
}

#[test]
fn t013_random_generator_gives_up_after_64_taken_candidates() {
    let calls = Cell::new(0_usize);
    let always_taken = |_: &Id| {
        calls.set(calls.get() + 1);
        true
    };

    let result = RandomIdGenerator::new().generate(IdKind::Feature, &always_taken);

    assert_eq!(result, Err(IdError::Exhausted { attempts: 64 }));
    assert_eq!(calls.get(), 64);
}

#[test]
fn t014_sequential_ids_have_the_generated_form() {
    let mut generator = SequentialIds::new();

    for kind in cycling_kinds(1000) {
        let generated = generator.generate(kind, &never_taken).expect("an id");

        assert!(
            Id::new(generated.as_str()).is_ok(),
            "{generated} is not a valid id"
        );
        assert!(
            has_generated_form(generated.as_str(), &["p-", "b-", "f-", "e-", "c-"]),
            "{generated} does not match ^[pbfec]-[0-9a-f]{{12}}$"
        );
    }
}

#[test]
fn t015_generators_work_as_trait_objects() {
    let mut sequential = SequentialIds::new();
    let mut random = RandomIdGenerator::new();
    let generators: [&mut dyn IdGenerator; 2] = [&mut sequential, &mut random];

    for generator in generators {
        let generated = generator
            .generate(IdKind::Body, &never_taken)
            .expect("an id");

        assert!(
            Id::new(generated.as_str()).is_ok(),
            "{generated} is not a valid id"
        );
    }
}

#[test]
fn t016_random_ids_are_well_formed_and_distinct() {
    let mut generator = RandomIdGenerator::new();
    let mut produced = BTreeSet::new();

    for _ in 0..10_000 {
        let generated = generator
            .generate(IdKind::Feature, &|candidate: &Id| {
                produced.contains(candidate)
            })
            .expect("the random source is available");

        assert!(
            has_generated_form(generated.as_str(), &["f-"]),
            "{generated} does not match ^f-[0-9a-f]{{12}}$"
        );
        assert!(produced.insert(generated), "an id was produced twice");
    }

    assert_eq!(produced.len(), 10_000);
}
