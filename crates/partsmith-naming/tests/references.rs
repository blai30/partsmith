//! Acceptance tests for references (spec section 6, tests 29 to 32).

mod common;

use common::{TOP_FINGERPRINT, box_scope, fingerprint, info_named, name, reference};
use partsmith_naming::{ElementKind, GeometryKind, NamingError, Reference};
use serde_json::json;

/// The first reference example of spec section 2.4, in the spec's spacing.
const END_CAP_REFERENCE: &str =
    r#"{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}"#;

#[test]
fn t029_reference_deserializes_with_face_query() {
    let parsed: Reference =
        serde_json::from_str(END_CAP_REFERENCE).expect("the spec example deserializes");

    assert_eq!(parsed.query, name("f-ext1/end-cap"));
    assert_eq!(parsed.query.kind(), ElementKind::Face);
    assert_eq!(
        parsed.fingerprint,
        fingerprint("plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4")
    );
}

#[test]
fn t029_reference_serializes_back_with_q_then_fp() {
    let parsed: Reference =
        serde_json::from_str(END_CAP_REFERENCE).expect("the spec example deserializes");

    let json = serde_json::to_string(&parsed).expect("a reference serializes");

    assert_eq!(
        json,
        r#"{"q":"f-ext1/end-cap","fp":"plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4"}"#
    );
}

#[test]
fn t030_deserializing_kind_mismatch_fails() {
    let expected = NamingError::KindMismatch {
        name: "edge(f-a/x|f-b/y)".to_owned(),
        kind: GeometryKind::Plane,
    };

    let error =
        serde_json::from_str::<Reference>(r#"{"q": "edge(f-a/x|f-b/y)", "fp": "plane|c=0,0,0"}"#)
            .expect_err("an edge query with a plane fingerprint must not deserialize");

    assert!(
        error.to_string().contains(&expected.to_string()),
        "the error must be KindMismatch ({expected}), got: {error}"
    );
}

#[test]
fn t030_new_with_kind_mismatch_fails() {
    let result = Reference::new(name("edge(f-a/x|f-b/y)"), fingerprint("plane|c=0,0,0"));

    assert_eq!(
        result,
        Err(NamingError::KindMismatch {
            name: "edge(f-a/x|f-b/y)".to_owned(),
            kind: GeometryKind::Plane,
        })
    );
}

#[test]
fn t031_reference_with_unknown_key_fails() {
    let result = serde_json::from_str::<Reference>(
        r#"{"q": "f-ext1/end-cap", "fp": "plane|n=0,0,1|c=40,25,30|a=4000|adj=4,4", "x": 1}"#,
    );

    assert!(result.is_err(), "an unknown key must be rejected");
}

#[test]
fn t031_reference_without_fingerprint_fails() {
    let result = serde_json::from_str::<Reference>(r#"{"q": "f-ext1/end-cap"}"#);

    assert!(result.is_err(), "a missing fp must be rejected");
}

#[test]
fn t032_from_info_on_box_top_face() {
    let scope = box_scope();
    let top = info_named(&scope, "f-box1/top");

    let made = Reference::from_info(top);

    assert_eq!(made, reference("f-box1/top", TOP_FINGERPRINT));
}

#[test]
fn t032_from_info_serializes_to_spec_json() {
    let scope = box_scope();
    let top = info_named(&scope, "f-box1/top");

    let value = serde_json::to_value(Reference::from_info(top)).expect("a reference serializes");

    assert_eq!(
        value,
        json!({"q": "f-box1/top", "fp": "plane|n=0,0,1|c=5,10,30|a=200|adj=4,4"})
    );
}
