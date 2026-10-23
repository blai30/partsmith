//! Feature statuses as the engine sends them (section 2.7), and the check that a status is one it sends.

use partsmith_protocol::{FeatureState, FeatureStatus, Repair, Severity};

use super::examples::{
    DELETED_POINT_MESSAGE, FEATURE_STATUS_R03, LOST_FACE_MESSAGE, LOST_FACE_REPAIR,
    REGION_AMBIGUOUS_MESSAGE, REGION_REPAIR,
};
use super::{id, parse};

/// The statuses of revision 2: the R03 fillet, whose cause and lost edge carry line ghosts, beside an extrude whose
/// region is ambiguous (test 68), whose repair carries the region outline.
pub fn edge_and_region_statuses() -> Vec<FeatureStatus> {
    sent_by_the_engine(vec![
        parse(FEATURE_STATUS_R03),
        FeatureStatus {
            id: id("f-ext1"),
            status: FeatureState::Warning,
            messages: vec![parse(REGION_AMBIGUOUS_MESSAGE)],
            repairs: vec![parse(REGION_REPAIR)],
        },
    ])
}

/// The statuses of revision 3: a datum plane whose base face was cut away, whose repair carries the face's triangles,
/// beside a hole whose point was deleted, whose cause carries the point.
pub fn face_and_point_statuses() -> Vec<FeatureStatus> {
    sent_by_the_engine(vec![
        FeatureStatus {
            id: id("f-dp1"),
            status: FeatureState::Error,
            messages: vec![parse(LOST_FACE_MESSAGE)],
            repairs: vec![parse(LOST_FACE_REPAIR)],
        },
        FeatureStatus {
            id: id("f-hole1"),
            status: FeatureState::Warning,
            messages: vec![parse(DELETED_POINT_MESSAGE)],
            repairs: Vec::new(),
        },
    ])
}

/// `statuses`, after checking each with `assert_sent_by_the_engine`.
fn sent_by_the_engine(statuses: Vec<FeatureStatus>) -> Vec<FeatureStatus> {
    for status in &statuses {
        assert_sent_by_the_engine(status);
    }
    statuses
}

/// Checks that the status of a feature that ran is one the engine sends (section 2.7): its messages from the most
/// severe down, its state the one its first message gives, and each repair at or inside the field of a message, in
/// message order.
pub fn assert_sent_by_the_engine(status: &FeatureStatus) {
    let severities: Vec<Severity> = status
        .messages
        .iter()
        .map(|message| message.severity)
        .collect();
    assert!(
        severities.is_sorted_by(|first, second| first >= second),
        "messages go from the most severe down: {status:?}"
    );
    let state = match severities.first() {
        Some(Severity::Error) => FeatureState::Error,
        Some(Severity::Warning) => FeatureState::Warning,
        Some(Severity::Info) | None => FeatureState::Ok,
    };
    assert_eq!(
        status.status, state,
        "the state is the one its first message gives: {status:?}"
    );
    let mut owner = 0;
    for repair in &status.repairs {
        let path = repair_path(repair);
        owner += status.messages[owner..]
            .iter()
            .position(|message| {
                message
                    .field
                    .as_deref()
                    .is_some_and(|field| lies_in(path, field))
            })
            .unwrap_or_else(|| {
                panic!("the repair at {path} belongs to a message, in message order: {status:?}")
            });
    }
}

/// The JSON Pointer of a repair.
fn repair_path(repair: &Repair) -> &str {
    let (Repair::Element { path, .. } | Repair::Region { path, .. }) = repair;
    path
}

/// Whether the JSON Pointer `path` is `field` or points inside it.
fn lies_in(path: &str, field: &str) -> bool {
    path.strip_prefix(field)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}
