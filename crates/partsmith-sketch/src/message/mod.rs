//! Messages of this crate's errors, solver issues and profile findings: the words for entities, points and
//! constraints, numbers in document units, and one-click fixes (spec section 4).

mod context;

use std::fmt;

use partsmith_model::Id;

pub use context::MessageContext;

use crate::point_ref::PointRef;

/// A one-click fix for a sketch message. Features turn it into a feature `Fix` (features.md section 2.8), and the
/// engine labels it (engine.md section 3.22). Not serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SketchFix {
    /// Delete this constraint.
    DeleteConstraint {
        /// The constraint to delete.
        constraint: Id,
    },
    /// Make this driving dimension a reference dimension.
    MakeReference {
        /// The driving dimension.
        constraint: Id,
    },
    /// Add a coincident constraint joining these two points.
    ConnectEnds {
        /// The open end the finding names.
        first: PointRef,
        /// The open end it is joined to.
        second: PointRef,
    },
}

/// `text` with its first letter capitalized, for words that begin a sentence.
pub(crate) fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// `{Type}` of a constraint type name: underscores become spaces and the first letter is capitalized
/// ("Point on object").
pub(crate) fn type_words(type_name: &str) -> String {
    capitalized(&type_name.replace('_', " "))
}

/// Words joined with `, ` and ` and ` before the last: `A`, `A and B`, `A, B and C`.
pub(crate) fn joined(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Ids joined by `,`, as details write lists (section 4.4).
pub(crate) struct IdList<'a>(pub(crate) &'a [Id]);

impl fmt::Display for IdList<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, id) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{id}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::id;

    #[test]
    fn capitalized_changes_only_the_first_letter() {
        assert_eq!(capitalized("the line from (0, 0)"), "The line from (0, 0)");
        assert_eq!(capitalized("Distance 5 mm"), "Distance 5 mm");
        assert_eq!(capitalized(""), "");
    }

    #[test]
    fn type_words_replace_underscores_and_capitalize() {
        assert_eq!(type_words("point_on_object"), "Point on object");
        assert_eq!(type_words("horizontal_distance"), "Horizontal distance");
        assert_eq!(type_words("fix"), "Fix");
    }

    #[test]
    fn joined_words_put_and_before_the_last() {
        let words = |texts: &[&str]| -> Vec<String> {
            texts.iter().map(|text| (*text).to_owned()).collect()
        };

        assert_eq!(joined(&words(&[])), "");
        assert_eq!(joined(&words(&["A"])), "A");
        assert_eq!(joined(&words(&["A", "B"])), "A and B");
        assert_eq!(joined(&words(&["A", "B", "C"])), "A, B and C");
    }

    #[test]
    fn id_lists_are_joined_by_commas() {
        assert_eq!(IdList(&[id("c-1"), id("c-2")]).to_string(), "c-1,c-2");
        assert_eq!(IdList(&[id("c-1")]).to_string(), "c-1");
        assert_eq!(IdList(&[]).to_string(), "");
    }
}
