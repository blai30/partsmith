//! Undo and redo of labeled transactions (model.md section 3.11).

use std::collections::VecDeque;

use crate::{Document, Edit, Transaction, TransactionError};

/// Undo and redo stacks of transactions for one document. Holds no reference to the document.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoLog {
    /// Transactions that can be undone, oldest first.
    undo: VecDeque<Transaction>,
    /// Transactions that can be redone, the next one to redo last.
    redo: Vec<Transaction>,
    /// Most transactions kept on the undo stack.
    max_depth: usize,
}

impl UndoLog {
    /// Default maximum number of transactions kept for undo.
    pub const DEFAULT_MAX_DEPTH: usize = 1000;

    /// An empty log with the default maximum depth.
    pub fn new() -> UndoLog {
        UndoLog::with_max_depth(UndoLog::DEFAULT_MAX_DEPTH)
    }

    /// An empty log keeping at most `max_depth` transactions (0 keeps none).
    pub fn with_max_depth(max_depth: usize) -> UndoLog {
        UndoLog {
            undo: VecDeque::new(),
            redo: Vec::new(),
            max_depth,
        }
    }

    /// The maximum depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Applies `edits` to `document` as one transaction, all or nothing, and records it.
    pub fn commit(
        &mut self,
        document: &mut Document,
        label: impl Into<String>,
        edits: Vec<Edit>,
    ) -> Result<(), TransactionError> {
        if edits.is_empty() {
            return Ok(());
        }
        let steps = document.apply_all(edits)?;
        self.record(Transaction {
            label: label.into(),
            steps,
        });
        Ok(())
    }

    /// Records a transaction whose steps the caller has already applied, in order, to the document.
    pub fn record(&mut self, transaction: Transaction) {
        if transaction.steps.is_empty() {
            return;
        }
        self.redo.clear();
        self.push_undo(transaction);
    }

    /// Undoes the newest transaction; `Ok(None)` when there is nothing to undo.
    pub fn undo(&mut self, document: &mut Document) -> Result<Option<&Transaction>, UndoError> {
        let Some(transaction) = self.undo.pop_back() else {
            return Ok(None);
        };
        let inverses = transaction
            .steps
            .iter()
            .rev()
            .map(|step| step.inverse.clone())
            .collect();
        if let Err(error) = document.apply_all(inverses) {
            let label = transaction.label.clone();
            self.undo.push_back(transaction);
            return Err(UndoError::Undo { label, error });
        }
        self.redo.push(transaction);
        Ok(self.redo.last())
    }

    /// Redoes the newest undone transaction; `Ok(None)` when there is nothing to redo.
    pub fn redo(&mut self, document: &mut Document) -> Result<Option<&Transaction>, UndoError> {
        let Some(transaction) = self.redo.pop() else {
            return Ok(None);
        };
        let edits = transaction
            .steps
            .iter()
            .map(|step| step.edit.clone())
            .collect();
        if let Err(error) = document.apply_all(edits) {
            let label = transaction.label.clone();
            self.redo.push(transaction);
            return Err(UndoError::Redo { label, error });
        }
        self.push_undo(transaction);
        Ok(self.undo.back())
    }

    /// True when a transaction can be undone.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// True when a transaction can be redone.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Label of the transaction `undo` would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo
            .back()
            .map(|transaction| transaction.label.as_str())
    }

    /// Label of the transaction `redo` would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo
            .last()
            .map(|transaction| transaction.label.as_str())
    }

    /// Number of transactions on the undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }

    /// Number of transactions on the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }

    /// Drops both stacks.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    /// Pushes onto the undo stack, then discards the oldest transactions beyond the maximum depth.
    fn push_undo(&mut self, transaction: Transaction) {
        self.undo.push_back(transaction);
        while self.undo.len() > self.max_depth {
            self.undo.pop_front();
        }
    }
}

impl Default for UndoLog {
    /// Same as `UndoLog::new()`.
    fn default() -> UndoLog {
        UndoLog::new()
    }
}

/// Why an undo or redo failed; the document and both stacks are unchanged.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum UndoError {
    /// An inverse no longer applies because the document changed outside the log.
    #[error("Cannot undo {label}: the document no longer matches the undo history. {error}")]
    Undo {
        /// Label of the transaction.
        label: String,
        /// The failure.
        error: TransactionError,
    },
    /// An edit no longer applies because the document changed outside the log.
    #[error("Cannot redo {label}: the document no longer matches the undo history. {error}")]
    Redo {
        /// Label of the transaction.
        label: String,
        /// The failure.
        error: TransactionError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EditError, Id, RecordKind, Units};

    fn set_units(units: Units) -> Vec<Edit> {
        vec![Edit::SetUnits { units }]
    }

    #[test]
    fn default_log_uses_the_default_depth() {
        assert_eq!(UndoLog::default(), UndoLog::new());
        assert_eq!(UndoLog::default().max_depth(), UndoLog::DEFAULT_MAX_DEPTH);
    }

    #[test]
    fn labels_follow_both_stacks() {
        let mut document = Document::default();
        let mut log = UndoLog::new();
        log.commit(&mut document, "A", set_units(Units::Cm))
            .expect("commit A");
        log.commit(&mut document, "B", set_units(Units::In))
            .expect("commit B");
        log.undo(&mut document).expect("undo B");

        assert_eq!(log.undo_label(), Some("A"));
        assert_eq!(log.redo_label(), Some("B"));
    }

    #[test]
    fn redo_keeps_the_rest_of_the_redo_stack() {
        let mut document = Document::default();
        let mut log = UndoLog::new();
        log.commit(&mut document, "A", set_units(Units::Cm))
            .expect("commit A");
        log.commit(&mut document, "B", set_units(Units::In))
            .expect("commit B");
        log.undo(&mut document).expect("undo B");
        log.undo(&mut document).expect("undo A");

        log.redo(&mut document).expect("redo A");

        assert_eq!(log.redo_label(), Some("B"));
        assert_eq!(document.units, Units::Cm);
    }

    #[test]
    fn a_failed_redo_changes_nothing() {
        let mut document = Document::default();
        let mut log = UndoLog::new();
        let parameter = crate::Parameter::new(Id::new("p-1").expect("valid id"), "a", "1");
        log.commit(
            &mut document,
            "Add a",
            vec![Edit::AddParameter {
                index: 0,
                parameter: parameter.clone(),
            }],
        )
        .expect("commit");
        log.undo(&mut document).expect("undo");
        document
            .apply(Edit::AddParameter {
                index: 0,
                parameter,
            })
            .expect("the id is free again");
        let before = document.clone();

        let result = log
            .redo(&mut document)
            .map(|transaction| transaction.cloned());

        assert!(matches!(
            result,
            Err(UndoError::Redo {
                error: TransactionError {
                    error: EditError::IdInUse { .. },
                    ..
                },
                ..
            })
        ));
        assert_eq!(document, before);
        assert_eq!(log.redo_count(), 1);
    }

    #[test]
    fn undo_error_message_names_the_transaction() {
        let error = UndoError::Undo {
            label: "Extrude".to_string(),
            error: TransactionError {
                index: 0,
                count: 1,
                error: EditError::NotFound {
                    kind: RecordKind::Feature,
                    id: Id::new("f-1").expect("valid id"),
                },
            },
        };

        assert_eq!(
            error.to_string(),
            "Cannot undo Extrude: the document no longer matches the undo history. Change 1 of 1 could not be applied: There is no feature with the id f-1."
        );
    }
}
