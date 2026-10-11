//! The canonical JSON writer and its helpers (file-format.md sections 3.3 to 3.5): one layout,
//! number form and string escaping for every `.ptsm` file and every other canonical text.

mod layout;
mod number;
mod parse;
mod string;
mod value;

pub use layout::to_string;
pub(crate) use layout::write_one_line;
pub(crate) use number::normalized;
pub use number::{format_number, normalize_numbers};
pub use parse::parse;
pub use string::format_string;
pub(crate) use value::to_finite_value;
pub use value::to_value;

/// Width limit for one-line containers, in characters.
pub const MAX_LINE_WIDTH: usize = 120;
/// Spaces per indentation level.
pub const INDENT_WIDTH: usize = 2;

/// Why a value cannot be turned into canonical JSON.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CanonicalError {
    /// A number is NaN or infinite.
    #[error("{path}: NaN and infinity cannot be saved. Use a finite number.")]
    NonFiniteNumber {
        /// Where the number is (file-format.md section 3.7), or `value` when it is the root.
        path: String,
    },
    /// The value's `Serialize` implementation failed or produced something JSON cannot hold.
    #[error("The value cannot be written as JSON: {message}")]
    Serialize {
        /// The serializer's description.
        message: String,
    },
}
