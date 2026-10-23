//! Helpers of the pass-through tests (`pass_through.rs`): [`side_by_side`] sends each request through `ps_call` and to a reference `Session` and compares the answers, [`commands`] builds the commands the tests send and the request sequences they read, [`documents`] the documents they open and the edits they apply, [`frames`] reads the frames that come back, and [`checks`] holds the assertions they share. A helper that one test alone uses stays beside that test.

mod checks;
mod commands;
mod documents;
mod frames;
mod side_by_side;

pub use checks::*;
pub use commands::*;
pub use documents::*;
pub use frames::*;
pub use side_by_side::*;
