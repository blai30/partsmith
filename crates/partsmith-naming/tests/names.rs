//! Acceptance tests for element names (spec section 6, tests 1 to 15).

mod common;

use common::{id, name};
use partsmith_naming::{ElementKind, ElementName, Generator, Instance, NamingError};

/// The valid examples of spec section 3.1.
const VALID_EXAMPLES: [&str; 11] = [
    "f-ext1/end-cap",
    "f-ext1/lateral/e-9e12",
    "f-ext1/lateral/e-1#2",
    "f-hole1/bore/e-1.center",
    "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)",
    "f-sh1/inner/f-ext1/lateral/e-3",
    "f-box1/top@f-pat1:2",
    "f-box1/top@f-pat1:0.1",
    "edge(f-cyl1/side)",
    "edge(f-box1/front|f-box1/top)#2",
    "vertex(f-box1/bottom|f-box1/front|f-box1/left)",
];

/// Asserts that parsing `text` fails with `InvalidName`.
fn assert_invalid_name(text: &str) {
    let result = ElementName::parse(text);
    assert!(
        matches!(result, Err(NamingError::InvalidName { .. })),
        "{text:?} must be rejected with InvalidName, got {result:?}"
    );
}

/// A face name of exactly `length` bytes: `f-a/` followed by a long role.
fn face_name_of_length(length: usize) -> String {
    format!("f-a/{}", "x".repeat(length - 4))
}

/// A vertex name whose `vertex(` groups nest `depth` levels deep, each level a face name whose
/// generator is the next vertex name.
fn nested_vertex_name(depth: usize) -> String {
    let mut text = String::from("vertex(f-a/x)");
    for _ in 1..depth {
        text = format!("vertex(f-a/r/{text})");
    }
    text
}

#[test]
fn t001_parse_accepts_valid_examples() {
    for text in VALID_EXAMPLES {
        let result = ElementName::parse(text);
        assert!(result.is_ok(), "{text:?} must parse, got {result:?}");
    }
}

#[test]
fn t001_as_str_returns_input_unchanged() {
    for text in VALID_EXAMPLES {
        assert_eq!(name(text).as_str(), text);
    }
}

#[test]
fn t002_parse_rejects_invalid_names() {
    let invalid = [
        "",
        "f-ext1",
        "f-ext1/",
        "f-ext1/End-cap",
        "f-ext1/end-cap#0",
        "f-ext1/end-cap#01",
        "f-ext1/end-cap#4294967296",
        "f ext1/end-cap",
        "edge()",
        "edge(f-a/x|f-a/x)",
        "edge(f-b/x|f-a/x)",
        "edge(f-a/x",
        "f-box1/top@f-p:0",
        "f-box1/top@f-p:01",
        "f-a/x/e-1.Center",
    ];
    for text in invalid {
        assert_invalid_name(text);
    }
}

#[test]
fn t002_parse_rejects_feature_id_of_65_characters() {
    assert_invalid_name(&format!("{}/x", "a".repeat(65)));
}

#[test]
fn t003_name_of_16384_bytes_parses() {
    let text = face_name_of_length(16384);
    assert_eq!(text.len(), 16384);

    let parsed = ElementName::parse(&text).map(|parsed| parsed.as_str().len());

    assert_eq!(parsed, Ok(16384));
}

#[test]
fn t003_name_of_16385_bytes_fails_naming_the_limit() {
    let text = face_name_of_length(16385);

    let result = ElementName::parse(&text);

    assert!(
        matches!(&result, Err(NamingError::InvalidName { reason, .. }) if reason.contains("16384")),
        "a 16385-byte name must fail with InvalidName mentioning 16384, got an error: {:?}",
        result.as_ref().err()
    );
}

#[test]
fn t003_vertex_name_nested_32_levels_parses() {
    let text = nested_vertex_name(32);
    assert_eq!(text.matches("vertex(").count(), 32);

    let result = ElementName::parse(&text);

    assert!(result.is_ok(), "32 levels must parse, got {result:?}");
}

#[test]
fn t003_vertex_name_nested_33_levels_fails() {
    let text = nested_vertex_name(33);
    assert_eq!(text.matches("vertex(").count(), 33);

    let result = ElementName::parse(&text);

    assert!(
        matches!(&result, Err(NamingError::InvalidName { reason, .. }) if reason.contains("32")),
        "33 levels must fail with InvalidName mentioning 32, got {result:?}"
    );
}

#[test]
fn t004_kind_follows_name_form() {
    assert_eq!(name("f-a/x").kind(), ElementKind::Face);
    assert_eq!(name("edge(f-a/x)").kind(), ElementKind::Edge);
    assert_eq!(name("vertex(f-a/x)").kind(), ElementKind::Vertex);
}

#[test]
fn t004_feature_named_edge_gives_face_name() {
    assert_eq!(name("edge/x").kind(), ElementKind::Face);
}

#[test]
fn t005_split_base_removes_every_trailing_split_suffix() {
    assert_eq!(
        name("f-x/top#2@f-p:1#1#3").split_base(),
        name("f-x/top#2@f-p:1")
    );
}

#[test]
fn t005_split_base_of_edge_name() {
    assert_eq!(
        name("edge(f-a/x|f-b/y)#2").split_base(),
        name("edge(f-a/x|f-b/y)")
    );
}

#[test]
fn t005_split_base_of_unsplit_name_is_itself() {
    assert_eq!(name("f-a/x").split_base(), name("f-a/x"));
}

#[test]
fn t006_split_suffixes_make_strict_descendants() {
    let descendant = name("f-a/x#1#2");

    assert!(descendant.is_split_descendant_of(&name("f-a/x")));
    assert!(descendant.is_split_descendant_of(&name("f-a/x#1")));
}

#[test]
fn t006_name_is_not_its_own_split_descendant() {
    let descendant = name("f-a/x#1#2");

    assert!(!descendant.is_split_descendant_of(&name("f-a/x#1#2")));
}

#[test]
fn t006_other_piece_is_not_an_ancestor() {
    assert!(!name("f-a/x#1#2").is_split_descendant_of(&name("f-a/x#2")));
}

#[test]
fn t006_instance_suffix_is_not_a_split() {
    assert!(!name("f-a/x@f-p:1").is_split_descendant_of(&name("f-a/x")));
}

#[test]
fn t007_names_with_split_suffix_are_order_dependent() {
    assert!(name("f-a/x#1").is_order_dependent());
    assert!(name("edge(f-a/x#1|f-b/y)").is_order_dependent());
}

#[test]
fn t007_instance_names_are_not_order_dependent() {
    assert!(!name("f-a/x@f-p:1").is_order_dependent());
}

#[test]
fn t008_face_terms_split_only_at_the_outer_level() {
    // The spec writes this name with its terms unsorted; the sorted form is the valid one.
    let edge = name("edge(f-c/z|f-fil1/fillet/edge(f-a/x|f-b/y))");

    assert_eq!(
        edge.face_terms(),
        vec![name("f-c/z"), name("f-fil1/fillet/edge(f-a/x|f-b/y)")]
    );
}

#[test]
fn t008_face_name_has_no_face_terms() {
    assert_eq!(name("f-a/x").face_terms(), Vec::<ElementName>::new());
}

#[test]
fn t009_generated_with_sketch_generator() {
    let generated =
        ElementName::generated(&id("f-ext1"), "lateral", Some(Generator::Sketch("e-9e12")));

    assert_eq!(generated, Ok(name("f-ext1/lateral/e-9e12")));
}

#[test]
fn t009_generated_without_generator() {
    let generated = ElementName::generated(&id("f-ext1"), "end-cap", None);

    assert_eq!(generated, Ok(name("f-ext1/end-cap")));
}

#[test]
fn t009_generated_rejects_role_with_uppercase() {
    let generated =
        ElementName::generated(&id("f-ext1"), "Lateral", Some(Generator::Sketch("e-9e12")));

    assert_eq!(
        generated,
        Err(NamingError::InvalidRole {
            role: "Lateral".to_owned()
        })
    );
}

#[test]
fn t009_generated_rejects_invalid_sketch_reference() {
    let generated =
        ElementName::generated(&id("f-ext1"), "lateral", Some(Generator::Sketch("e 1")));

    assert_eq!(
        generated,
        Err(NamingError::InvalidGenerator {
            text: "e 1".to_owned()
        })
    );
}

#[test]
fn t010_generated_with_element_generator() {
    let edge = name("edge(f-ext1/end-cap|f-ext1/lateral/e-1)");

    let generated =
        ElementName::generated(&id("f-fil1"), "fillet", Some(Generator::Element(&edge)));

    assert_eq!(
        generated,
        Ok(name(
            "f-fil1/fillet/edge(f-ext1/end-cap|f-ext1/lateral/e-1)"
        ))
    );
}

#[test]
fn t011_edge_sorts_and_deduplicates_faces() {
    let edge = ElementName::edge(&[name("f-box1/top"), name("f-box1/front"), name("f-box1/top")]);

    assert_eq!(edge, Ok(name("edge(f-box1/front|f-box1/top)")));
}

#[test]
fn t011_vertex_sorts_faces() {
    let vertex = ElementName::vertex(&[
        name("f-box1/top"),
        name("f-box1/left"),
        name("f-box1/front"),
    ]);

    assert_eq!(
        vertex,
        Ok(name("vertex(f-box1/front|f-box1/left|f-box1/top)"))
    );
}

#[test]
fn t012_with_split_appends_suffix() {
    assert_eq!(name("f-a/x#1").with_split(2), Ok(name("f-a/x#1#2")));
}

#[test]
fn t012_with_split_rejects_zero() {
    let result = name("f-a/x#1").with_split(0);

    assert!(
        matches!(result, Err(NamingError::InvalidName { .. })),
        "split 0 must fail with InvalidName, got {result:?}"
    );
}

#[test]
fn t013_with_instance_one_direction() {
    let renamed = name("f-box1/top#1").with_instance(
        &id("f-pat1"),
        Instance {
            first: 2,
            second: 0,
        },
    );

    assert_eq!(renamed, Ok(name("f-box1/top#1@f-pat1:2")));
}

#[test]
fn t013_with_instance_first_index_zero() {
    let renamed = name("f-box1/top#1").with_instance(
        &id("f-pat1"),
        Instance {
            first: 0,
            second: 3,
        },
    );

    assert_eq!(renamed, Ok(name("f-box1/top#1@f-pat1:0.3")));
}

#[test]
fn t013_with_instance_two_directions() {
    let renamed = name("f-box1/top#1").with_instance(
        &id("f-pat1"),
        Instance {
            first: 1,
            second: 1,
        },
    );

    assert_eq!(renamed, Ok(name("f-box1/top#1@f-pat1:1.1")));
}

#[test]
fn t013_with_instance_rejects_original() {
    let renamed = name("f-box1/top#1").with_instance(
        &id("f-pat1"),
        Instance {
            first: 0,
            second: 0,
        },
    );

    assert_eq!(renamed, Err(NamingError::InvalidInstance));
}

#[test]
fn t014_with_instance_on_edge_renames_and_resorts_terms() {
    let renamed = name("edge(f-x/side|f-x/side-2)#1").with_instance(
        &id("f-p"),
        Instance {
            first: 1,
            second: 0,
        },
    );

    assert_eq!(renamed, Ok(name("edge(f-x/side-2@f-p:1|f-x/side@f-p:1)#1")));
}

#[test]
fn t015_name_serializes_as_json_string() {
    let original = name("f-ext1/end-cap#2");

    let json = serde_json::to_string(&original).expect("a name serializes");
    let back: ElementName = serde_json::from_str(&json).expect("a valid name deserializes");

    assert_eq!(json, "\"f-ext1/end-cap#2\"");
    assert_eq!(back, original);
}

#[test]
fn t015_deserializing_invalid_name_fails() {
    let result = serde_json::from_str::<ElementName>("\"f-ext1/End\"");

    assert!(result.is_err(), "\"f-ext1/End\" must not deserialize");
}

#[test]
fn t015_ordering_is_byte_order() {
    assert!(name("f-a/x") < name("f-a/x#1"));
    assert!(name("f-a/x#1") < name("f-a/x-y"));
    assert!(name("f-a/x-y") < name("f-a/x@f-p:1"));
}
