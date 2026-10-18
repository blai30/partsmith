//! The state of one open document: recipe, revision, undo log, transaction, drags, dirty
//! baseline, assets, display quality, the latest recompute, what was last emitted and the
//! memoized inspections of its records (engine.md sections 3.2 to 3.6 and 3.25); and the canonical forms of a recipe: its text, and its params as the
//! registry writes them.

use std::collections::BTreeMap;
use std::sync::Arc;

use partsmith_features::Registry;
use partsmith_mesh::TessellationTolerance;
use partsmith_model::{Document, Id, Step, UndoLog, canonical, save};
use partsmith_protocol::TessellationQuality;

use crate::error::EngineError;
use crate::events::Emitted;
use crate::profiles::tolerance;
use crate::sketch::Drag;
use crate::state::Computed;
use crate::units::Inspections;

/// An open transaction: its label, the recorded edits and the focus feature.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OpenTransaction {
    /// The undo label.
    pub(crate) label: String,
    /// Recorded `(edit, inverse)` pairs, coalesced (section 3.3).
    pub(crate) steps: Vec<Step>,
    /// The focus feature.
    pub(crate) focus: Option<Id>,
}

/// The text and revision of the latest `save_document`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedText {
    /// The revision the text describes.
    pub(crate) revision: u64,
    /// The canonical text.
    pub(crate) text: String,
}

/// One open document.
#[derive(Debug)]
pub(crate) struct OpenDocument {
    /// The current recipe, including uncommitted transaction edits.
    pub(crate) document: Arc<Document>,
    /// Increases by one on every recipe change.
    pub(crate) revision: u64,
    /// Committed transactions.
    pub(crate) undo: UndoLog,
    /// The open transaction.
    pub(crate) transaction: Option<OpenTransaction>,
    /// Drag states by sketch feature.
    pub(crate) drags: BTreeMap<Id, Drag>,
    /// Canonical text the dirty state compares against.
    pub(crate) baseline: String,
    /// The latest `save_document`.
    pub(crate) last_save: Option<SavedText>,
    /// Canonical text of the recipe at a revision, computed on demand.
    text: SavedText,
    /// Stored asset bytes by name.
    pub(crate) assets: BTreeMap<String, Arc<[u8]>>,
    /// Display tessellation quality.
    pub(crate) quality: TessellationQuality,
    /// The latest recompute.
    pub(crate) computed: Computed,
    /// What the latest events described.
    pub(crate) emitted: Emitted,
    /// The recipe as of the last successful command, once a command panicked (section 3.15).
    pub(crate) poisoned: Option<Arc<Document>>,
    /// The registry's inspection of each feature record, memoized for `bare_numbers` and Keep
    /// sizes (section 3.25).
    pub(crate) inspections: Inspections,
    /// The latest count of `bare_numbers` with the revision it counts.
    pub(crate) bare_count: Option<(u64, u32)>,
}

impl OpenDocument {
    /// A document whose dirty baseline is its own canonical text.
    pub(crate) fn new(
        document: Document,
        assets: BTreeMap<String, Arc<[u8]>>,
        undo_limit: usize,
        quality: TessellationQuality,
        computed: Computed,
    ) -> Result<OpenDocument, EngineError> {
        let baseline = canonical_text(&document)?;
        Ok(OpenDocument {
            text: SavedText {
                revision: 0,
                text: baseline.clone(),
            },
            document: Arc::new(document),
            revision: 0,
            undo: UndoLog::with_max_depth(undo_limit),
            transaction: None,
            drags: BTreeMap::new(),
            baseline,
            last_save: None,
            assets,
            quality,
            computed,
            emitted: Emitted::default(),
            poisoned: None,
            inspections: Inspections::default(),
            bare_count: None,
        })
    }

    /// The canonical text of the current recipe.
    pub(crate) fn text(&mut self) -> Result<&str, EngineError> {
        self.refresh_text()?;
        Ok(&self.text.text)
    }

    /// True when the canonical text of the current recipe differs from the baseline.
    pub(crate) fn dirty(&mut self) -> Result<bool, EngineError> {
        self.refresh_text()?;
        Ok(self.text.text != self.baseline)
    }

    /// Computes the canonical text when the revision changed since it was last computed.
    fn refresh_text(&mut self) -> Result<(), EngineError> {
        if self.text.revision != self.revision {
            self.text.text = canonical_text(&self.document)?;
            self.text.revision = self.revision;
        }
        Ok(())
    }

    /// The tolerance of the document's display meshes.
    pub(crate) fn display_tolerance(&self) -> TessellationTolerance {
        tolerance(&self.computed.profile.profile, self.quality)
    }

    /// The focus feature of the open transaction.
    pub(crate) fn focus(&self) -> Option<&Id> {
        self.transaction
            .as_ref()
            .and_then(|transaction| transaction.focus.as_ref())
    }

    /// Fails with `transaction_open` while a transaction is open; `then` says what the failed
    /// request does, for the error's action (section 4).
    pub(crate) fn require_no_transaction(&self, then: &str) -> Result<(), EngineError> {
        match &self.transaction {
            Some(transaction) => Err(EngineError::TransactionOpen {
                label: transaction.label.clone(),
                then: then.to_string(),
            }),
            None => Ok(()),
        }
    }

    /// Replaces the recipe after a change: bumps the revision and discards every drag state.
    pub(crate) fn change(&mut self, document: Document) {
        self.document = Arc::new(document);
        self.revision += 1;
        self.drags.clear();
    }
}

/// The canonical `.ptsm` text of a recipe the engine holds (always structurally valid).
pub(crate) fn canonical_text(document: &Document) -> Result<String, EngineError> {
    save(document).map_err(EngineError::unexpected)
}

/// Replaces the params of every record the registry accepts by its canonical params; records it
/// rejects keep their params exactly as loaded.
pub(crate) fn canonicalize_params(registry: &Registry, document: &mut Document) {
    for record in &mut document.features {
        if let Ok(validated) = registry.validate_and_canonicalize(record) {
            record.params = validated.record.params;
            for value in record.params.values_mut() {
                canonical::normalize_numbers(value);
            }
        }
    }
}
