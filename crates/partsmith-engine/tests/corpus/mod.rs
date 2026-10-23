//! Documents of the reference stability corpus (`docs/design/reference-stability-corpus.md`) and of the newcomer error
//! corpus that engine.md section 6 tests 115 to 153 open, written as `.ptsm` texts from the corpus's shared setups; the
//! picks their setups store; their edits; and readers of the statuses, actions and repairs those tests check.
//!
//! A setup stores its references as picks store them (the corpus's "Picked references" rule): [`picked`] opens it in
//! a headless session, focuses each feature that stores a reference, so that the display state is the state just
//! before that feature (engine.md section 3.5), stores the full reference `pick_reference` gives for each element the
//! feature names (section 3.9) and the reference `solve_sketch` gives for each profile region (`Region::reference()`,
//! with its outline), marks each face sketch's fallback `fallback_frame: body`, and returns the saved text. "After its
//! edit" is the setup opened, then the corpus edit applied as one labeled `apply_edits` ([`after_edit`]).
//!
//! [`records`] writes the records and the shared setups, [`scenarios`] the corpus scenarios and the other edited
//! documents the tests open, [`newcomer`] the newcomer error corpus, [`picks`] the picks, [`edits`] the edits and the
//! documents after them, and [`statuses`] reads what the tests check.

mod edits;
mod newcomer;
mod picks;
mod records;
mod scenarios;
mod statuses;

pub use edits::*;
pub use newcomer::*;
pub use picks::*;
pub use records::*;
pub use scenarios::*;
pub use statuses::*;
