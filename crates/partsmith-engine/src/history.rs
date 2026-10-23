//! A document's undo history (engine.md section 3.3) with a serial number for each entry, so that
//! a cause remembers the entry whose command made its change and offers `Undo` only while that
//! entry is the newest, and with a count of the commands that changed the recipe, so that it
//! offers `Undo` only when that command made every change since its clean snapshot (section 3.21,
//! cause action 3).

use std::collections::VecDeque;

use partsmith_model::{Document, Transaction, UndoError, UndoLog};

/// The undo log of a document and the serial number of each of its entries.
#[derive(Debug)]
pub(crate) struct History {
    /// The undo and redo stacks.
    log: UndoLog,
    /// The serial of each undo entry, oldest first, as the log keeps them.
    undo: VecDeque<u64>,
    /// The serial of each redo entry, the next one to redo last.
    redo: Vec<u64>,
    /// The serial the next entry or transaction gets.
    next: u64,
    /// The entry whose command made the latest change of the recipe: the entry recorded or
    /// redone, or the open transaction's; `None` after an undo or a canceled transaction, whose
    /// change no undo entry made.
    changed_by: Option<u64>,
    /// How many commands have changed the recipe: each change made by another entry than the
    /// change before it, and each undo and canceled transaction, counts one, so the edits of one
    /// transaction count once.
    commands: u64,
}

impl History {
    /// An empty history keeping at most `max_depth` entries, as `UndoLog::with_max_depth`.
    pub(crate) fn new(max_depth: usize) -> History {
        History {
            log: UndoLog::with_max_depth(max_depth),
            undo: VecDeque::new(),
            redo: Vec::new(),
            next: 1,
            changed_by: None,
            commands: 0,
        }
    }

    /// A new serial, for an entry recorded now or for a transaction recorded when it commits.
    pub(crate) fn serial(&mut self) -> u64 {
        let serial = self.next;
        self.next += 1;
        serial
    }

    /// Records `transaction` as the newest entry with `serial` (nothing when it has no steps), and
    /// clears the redo stack.
    pub(crate) fn record(&mut self, serial: u64, transaction: Transaction) {
        if transaction.steps.is_empty() {
            return;
        }
        self.log.record(transaction);
        self.redo.clear();
        self.push_undo(serial);
    }

    /// Notes that the latest change of the recipe was made by the entry `serial`, or by no
    /// entry, and counts a command when the entry differs from the one of the change before.
    pub(crate) fn changed_by(&mut self, serial: Option<u64>) {
        if serial.is_none() || serial != self.changed_by {
            self.commands += 1;
        }
        self.changed_by = serial;
    }

    /// The entry whose command made the latest change of the recipe, if one did.
    pub(crate) fn latest_change(&self) -> Option<u64> {
        self.changed_by
    }

    /// How many commands have changed the recipe so far.
    pub(crate) fn commands(&self) -> u64 {
        self.commands
    }

    /// The serial of the newest undo entry.
    pub(crate) fn newest(&self) -> Option<u64> {
        self.undo.back().copied()
    }

    /// Undoes the newest entry on `document`; its label, or `None` when there is nothing to undo.
    pub(crate) fn undo(&mut self, document: &mut Document) -> Result<Option<String>, UndoError> {
        let label = self
            .log
            .undo(document)?
            .map(|transaction| transaction.label.clone());
        if label.is_some() {
            if let Some(serial) = self.undo.pop_back() {
                self.redo.push(serial);
            }
            self.changed_by(None);
        }
        Ok(label)
    }

    /// Redoes the newest undone entry on `document`; its label, or `None` when there is nothing
    /// to redo.
    pub(crate) fn redo(&mut self, document: &mut Document) -> Result<Option<String>, UndoError> {
        let label = self
            .log
            .redo(document)?
            .map(|transaction| transaction.label.clone());
        if label.is_some()
            && let Some(serial) = self.redo.pop()
        {
            self.push_undo(serial);
            self.changed_by(Some(serial));
        }
        Ok(label)
    }

    /// The newest undo entry's label.
    pub(crate) fn undo_label(&self) -> Option<&str> {
        self.log.undo_label()
    }

    /// The newest redo entry's label.
    pub(crate) fn redo_label(&self) -> Option<&str> {
        self.log.redo_label()
    }

    /// Pushes an entry's serial, dropping the oldest beyond the log's depth as the log does.
    fn push_undo(&mut self, serial: u64) {
        self.undo.push_back(serial);
        while self.undo.len() > self.log.max_depth() {
            self.undo.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use partsmith_model::{Edit, Parameter, Step, Units};

    use super::*;
    use crate::test_support::id;

    /// A transaction that sets the parameter `p-h` from `before` to `after`.
    fn set_height(before: &str, after: &str) -> Transaction {
        Transaction {
            label: format!("Height {after}"),
            steps: vec![Step {
                edit: Edit::UpdateParameter {
                    parameter: Parameter::new(id("p-h"), "height", after),
                },
                inverse: Edit::UpdateParameter {
                    parameter: Parameter::new(id("p-h"), "height", before),
                },
            }],
        }
    }

    /// A document with the parameter `p-h` at `expr`.
    fn document(expr: &str) -> Document {
        let mut document = Document::new(Units::Mm);
        document
            .parameters
            .push(Parameter::new(id("p-h"), "height", expr));
        document
    }

    #[test]
    fn serials_follow_their_entries_through_undo_and_redo() {
        let mut history = History::new(10);
        let mut recipe = document("2 mm");
        let first = history.serial();
        history.record(first, set_height("1 mm", "2 mm"));
        let second = history.serial();
        history.record(second, set_height("2 mm", "3 mm"));

        assert_eq!(history.newest(), Some(second));
        assert_eq!(
            history.undo(&mut recipe).expect("the edit undoes"),
            Some("Height 3 mm".to_string())
        );
        assert_eq!(history.newest(), Some(first));
        assert_eq!(history.latest_change(), None);
        assert_eq!(
            history.redo(&mut recipe).expect("the edit redoes"),
            Some("Height 3 mm".to_string())
        );
        assert_eq!(history.newest(), Some(second));
        assert_eq!(history.latest_change(), Some(second));
    }

    #[test]
    fn the_oldest_serial_goes_with_the_oldest_entry() {
        let mut history = History::new(1);
        let mut recipe = document("3 mm");
        let first = history.serial();
        history.record(first, set_height("1 mm", "2 mm"));
        let second = history.serial();
        history.record(second, set_height("2 mm", "3 mm"));

        history.undo(&mut recipe).expect("the edit undoes");

        assert_eq!(history.newest(), None);
        assert_eq!(history.undo(&mut recipe).expect("nothing is left"), None);
    }

    #[test]
    fn the_edits_of_one_entry_count_as_one_command() {
        let mut history = History::new(10);
        let mut recipe = document("2 mm");
        let transaction = history.serial();
        history.changed_by(Some(transaction));
        history.changed_by(Some(transaction));
        history.record(transaction, set_height("1 mm", "2 mm"));
        assert_eq!(history.commands(), 1);

        let edit = history.serial();
        history.changed_by(Some(edit));
        history.record(edit, set_height("2 mm", "3 mm"));
        history.undo(&mut recipe).expect("the edit undoes");
        history.redo(&mut recipe).expect("the edit redoes");

        // The edit, its undo and its redo are a command each.
        assert_eq!(history.commands(), 4);
        history.changed_by(None);
        assert_eq!(history.commands(), 5);
    }

    #[test]
    fn nothing_to_undo_counts_no_command() {
        let mut history = History::new(10);
        let mut recipe = document("2 mm");

        history.undo(&mut recipe).expect("nothing is no error");

        assert_eq!(history.commands(), 0);
    }

    #[test]
    fn an_empty_transaction_records_no_entry() {
        let mut history = History::new(10);
        let serial = history.serial();

        history.record(
            serial,
            Transaction {
                label: "Nothing".to_string(),
                steps: Vec::new(),
            },
        );

        assert_eq!(history.newest(), None);
        assert_eq!(history.undo_label(), None);
    }
}
