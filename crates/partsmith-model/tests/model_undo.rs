//! Acceptance tests for transactions and the undo log (model.md section 6, "Transactions and undo", tests 74 to 86).

use crate::common;

use common::{d1, feature, id};
use partsmith_model::{
    Edit, EditError, Parameter, RecordKind, Transaction, TransactionError, UndoError, UndoLog,
    Units,
};
use serde_json::json;

/// Adds parameter `p-d` named `name` at the end of D1's parameters.
fn add_parameter(name: &str) -> Edit {
    Edit::AddParameter {
        index: 2,
        parameter: Parameter::new(id("p-d"), name, "5 mm"),
    }
}

/// Sets the document units.
fn set_units(units: Units) -> Edit {
    Edit::SetUnits { units }
}

/// The label of the transaction an undo or redo returned.
fn label_of(result: Result<Option<&Transaction>, UndoError>) -> Result<Option<String>, UndoError> {
    result.map(|transaction| transaction.map(|transaction| transaction.label.clone()))
}

#[test]
fn t074_apply_all_returns_each_step_with_its_inverse() {
    let mut document = d1();

    let steps = document
        .apply_all(vec![add_parameter("depth"), set_units(Units::In)])
        .expect("both edits apply");

    let inverses: Vec<Edit> = steps.into_iter().map(|step| step.inverse).collect();
    assert_eq!(
        inverses,
        vec![
            Edit::RemoveParameter { id: id("p-d") },
            set_units(Units::Mm)
        ]
    );
}

#[test]
fn t075_apply_all_is_all_or_nothing() {
    let mut document = d1();
    let duplicate = Edit::AddParameter {
        index: 3,
        parameter: Parameter::new(id("p-e"), "depth", "1"),
    };

    let result = document.apply_all(vec![
        set_units(Units::In),
        add_parameter("depth"),
        duplicate,
    ]);

    let error = result.expect_err("the third edit reuses the name depth");
    assert_eq!(
        error,
        TransactionError {
            index: 2,
            count: 3,
            error: EditError::NameInUse {
                name: "depth".to_string()
            },
        }
    );
    assert_eq!(document, d1());
    assert_eq!(
        error.to_string(),
        "Change 3 of 3 could not be applied: A parameter named depth already exists. Pick another name."
    );
}

#[test]
fn t076_commit_undo_and_redo() {
    let mut document = d1();
    let mut log = UndoLog::new();

    log.commit(&mut document, "Add depth", vec![add_parameter("depth")])
        .expect("the commit applies");
    let after_commit = document.clone();
    assert_eq!(log.undo_label(), Some("Add depth"));

    let undone = label_of(log.undo(&mut document));
    assert_eq!(undone, Ok(Some("Add depth".to_string())));
    assert_eq!(document, d1());
    assert_eq!(log.redo_label(), Some("Add depth"));

    let redone = label_of(log.redo(&mut document));
    assert_eq!(redone, Ok(Some("Add depth".to_string())));
    assert_eq!(document, after_commit);
    assert!(!log.can_redo());
}

#[test]
fn t077_undo_applies_inverses_in_reverse_order() {
    let mut document = d1();
    let mut log = UndoLog::new();
    let rename = Edit::UpdateParameter {
        parameter: Parameter::new(id("p-d"), "b", "5 mm"),
    };

    log.commit(
        &mut document,
        "Add and rename",
        vec![add_parameter("a"), rename],
    )
    .expect("the commit applies");
    log.undo(&mut document).expect("the undo applies");

    assert_eq!(document, d1());
}

#[test]
fn t078_a_new_commit_clears_redo() {
    let mut document = d1();
    let mut log = UndoLog::new();

    log.commit(&mut document, "A", vec![set_units(Units::Cm)])
        .expect("commit A");
    log.commit(&mut document, "B", vec![set_units(Units::In)])
        .expect("commit B");
    log.undo(&mut document).expect("undo B");
    log.commit(&mut document, "C", vec![set_units(Units::M)])
        .expect("commit C");

    assert!(!log.can_redo());
    assert_eq!(log.redo_count(), 0);
}

#[test]
fn t079_a_failed_commit_keeps_both_stacks() {
    let mut document = d1();
    let mut log = UndoLog::new();
    log.commit(&mut document, "A", vec![set_units(Units::Cm)])
        .expect("commit A");
    log.undo(&mut document).expect("undo A");
    let before = document.clone();

    let result = log.commit(
        &mut document,
        "Broken",
        vec![Edit::RemoveParameter { id: id("p-zz") }],
    );

    assert_eq!(
        result,
        Err(TransactionError {
            index: 0,
            count: 1,
            error: EditError::NotFound {
                kind: RecordKind::Parameter,
                id: id("p-zz")
            },
        })
    );
    assert_eq!(log.redo_count(), 1);
    assert_eq!(log.undo_count(), 0);
    assert_eq!(document, before);
}

#[test]
fn t080_an_empty_commit_changes_nothing() {
    let mut document = d1();
    let mut log = UndoLog::new();
    log.commit(&mut document, "A", vec![set_units(Units::Cm)])
        .expect("commit A");
    log.commit(&mut document, "B", vec![set_units(Units::In)])
        .expect("commit B");
    log.undo(&mut document).expect("undo B");
    let before = document.clone();

    let result = log.commit(&mut document, "Nothing", vec![]);

    assert_eq!(result, Ok(()));
    assert_eq!(document, before);
    assert_eq!(log.undo_count(), 1);
    assert_eq!(log.redo_count(), 1);
}

#[test]
fn t081_maximum_depth_discards_the_oldest_transactions() {
    let mut document = d1();
    let mut log = UndoLog::with_max_depth(3);
    for units in [Units::Cm, Units::M, Units::In, Units::Mm, Units::Cm] {
        log.commit(&mut document, "Set units", vec![set_units(units)])
            .expect("the commit applies");
    }

    assert_eq!(log.undo_count(), 3);
    for expected in [Units::Mm, Units::In, Units::M] {
        let undone = label_of(log.undo(&mut document));
        assert_eq!(undone, Ok(Some("Set units".to_string())));
        assert_eq!(document.units, expected);
    }
    assert_eq!(label_of(log.undo(&mut document)), Ok(None));
    assert_eq!(document.units, Units::M);
}

#[test]
fn t082_maximum_depth_zero_keeps_nothing() {
    let mut document = d1();
    let mut log = UndoLog::with_max_depth(0);

    log.commit(&mut document, "Set units", vec![set_units(Units::In)])
        .expect("the commit applies");

    assert_eq!(document.units, Units::In);
    assert!(!log.can_undo());
    assert_eq!(label_of(log.undo(&mut document)), Ok(None));
}

#[test]
fn t083_a_new_log_is_empty() {
    let mut document = d1();
    let mut log = UndoLog::new();

    assert_eq!(label_of(log.undo(&mut document)), Ok(None));
    assert_eq!(label_of(log.redo(&mut document)), Ok(None));
    assert_eq!(UndoLog::new().max_depth(), 1000);
}

#[test]
fn t084_undo_reports_a_document_changed_outside_the_log() {
    let mut document = d1();
    let mut log = UndoLog::new();
    log.commit(
        &mut document,
        "Add box",
        vec![Edit::AddFeature {
            index: 3,
            feature: feature("f-new", "box", json!({})),
        }],
    )
    .expect("the commit applies");
    document
        .apply(Edit::RemoveFeature { id: id("f-new") })
        .expect("the feature exists");
    let before = document.clone();

    let result = label_of(log.undo(&mut document));

    assert_eq!(
        result,
        Err(UndoError::Undo {
            label: "Add box".to_string(),
            error: TransactionError {
                index: 0,
                count: 1,
                error: EditError::NotFound {
                    kind: RecordKind::Feature,
                    id: id("f-new")
                },
            },
        })
    );
    assert_eq!(document, before);
    assert_eq!(log.undo_count(), 1);
}

#[test]
fn t085_clear_empties_both_stacks_and_keeps_the_depth() {
    let mut document = d1();
    let mut log = UndoLog::with_max_depth(5);
    log.commit(&mut document, "A", vec![set_units(Units::Cm)])
        .expect("commit A");
    log.commit(&mut document, "B", vec![set_units(Units::In)])
        .expect("commit B");
    log.undo(&mut document).expect("undo B");

    log.clear();

    assert!(!log.can_undo());
    assert!(!log.can_redo());
    assert_eq!(log.max_depth(), 5);
}

#[test]
fn t086_record_adds_a_transaction_applied_by_the_caller() {
    let mut document = d1();
    let mut log = UndoLog::new();
    log.commit(
        &mut document,
        "Set printer",
        vec![Edit::SetPrinter {
            printer: Some("generic-fdm".to_string()),
        }],
    )
    .expect("the commit applies");
    log.undo(&mut document).expect("the undo applies");
    assert_eq!(document, d1());
    assert_eq!(log.redo_count(), 1);

    let steps = document
        .apply_all(vec![add_parameter("depth"), set_units(Units::In)])
        .expect("both edits apply");
    log.record(Transaction {
        label: "Two steps".to_string(),
        steps,
    });

    assert_eq!(log.redo_count(), 0);
    assert_eq!(log.undo_count(), 1);
    assert_eq!(log.undo_label(), Some("Two steps"));
    log.undo(&mut document).expect("the undo applies");
    assert_eq!(document, d1());

    let undo_count = log.undo_count();
    let redo_count = log.redo_count();
    log.record(Transaction {
        label: "Nothing".to_string(),
        steps: vec![],
    });
    assert_eq!(log.undo_count(), undo_count);
    assert_eq!(log.redo_count(), redo_count);
}
