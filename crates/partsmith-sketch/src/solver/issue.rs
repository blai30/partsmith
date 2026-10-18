//! Solver issues: their messages and fixes (spec section 4.2) and their details (spec section 4.4).

use std::fmt;

use partsmith_model::Id;

use crate::message::{IdList, MessageContext, SketchFix, joined};
use crate::params::SketchParams;

/// A user-facing solver finding. `render` gives the message of section 4.2 and `Display` the detail of section 4.4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolveIssue {
    /// These constraints cannot all hold.
    Conflicting {
        /// The conflicting constraints in document order.
        constraints: Vec<Id>,
    },
    /// This constraint repeats what others already determine.
    Redundant {
        /// The redundant constraint.
        constraint: Id,
        /// Its JSON type name.
        type_name: &'static str,
        /// True for a driving dimension.
        dimension: bool,
    },
    /// Some component did not converge and no conflict explains it.
    NotConverged,
}

impl SolveIssue {
    /// The message (section 4.2). A conflict lists its constraints newest first, in reverse document order.
    pub fn render(&self, context: &MessageContext<'_>) -> String {
        let label = context.label;
        match self {
            SolveIssue::Conflicting { constraints } => {
                let newest_first: Vec<&Id> = constraints.iter().rev().collect();
                match context.titles(&newest_first).split_first() {
                    Some((only, [])) => format!("In '{label}', {only} cannot hold."),
                    Some((first, others)) => format!(
                        "In '{label}', {first} cannot hold together with {}.",
                        joined(others)
                    ),
                    None => format!("In '{label}', the constraints cannot all hold."),
                }
            }
            SolveIssue::Redundant {
                constraint,
                type_name,
                dimension,
            } => {
                let title = context.title_or(constraint, Some(type_name));
                let verb = if *dimension { "set" } else { "hold" };
                format!(
                    "In '{label}', {title} is not needed: the other constraints already {verb} it."
                )
            }
            SolveIssue::NotConverged => format!(
                "'{label}' could not be solved from its current shape. Undo the last change, or drag the geometry closer to the intended shape."
            ),
        }
    }

    /// One-click fixes in the order hosts offer them (section 4.2); `sketch` must be the sketch that was solved.
    /// A conflict offers, for each constraint newest first, to delete it and, for a driving dimension, to make it
    /// a reference; hosts offer Undo for `NotConverged`.
    pub fn fixes(&self, sketch: &SketchParams) -> Vec<SketchFix> {
        match self {
            SolveIssue::Conflicting { constraints } => constraints
                .iter()
                .rev()
                .flat_map(|constraint| {
                    let driving = sketch
                        .constraint(constraint)
                        .is_some_and(|found| found.is_driving_dimension());
                    let delete = SketchFix::DeleteConstraint {
                        constraint: constraint.clone(),
                    };
                    let reference = driving.then(|| SketchFix::MakeReference {
                        constraint: constraint.clone(),
                    });
                    std::iter::once(delete).chain(reference)
                })
                .collect(),
            SolveIssue::Redundant {
                constraint,
                dimension,
                ..
            } => {
                let delete = SketchFix::DeleteConstraint {
                    constraint: constraint.clone(),
                };
                if *dimension {
                    vec![
                        SketchFix::MakeReference {
                            constraint: constraint.clone(),
                        },
                        delete,
                    ]
                } else {
                    vec![delete]
                }
            }
            SolveIssue::NotConverged => Vec::new(),
        }
    }
}

/// The detail of section 4.4, for example `conflicting constraints=c-1,c-2`.
impl fmt::Display for SolveIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveIssue::Conflicting { constraints } => {
                write!(formatter, "conflicting constraints={}", IdList(constraints))
            }
            SolveIssue::Redundant {
                constraint,
                type_name,
                dimension,
            } => write!(
                formatter,
                "redundant constraint={constraint} type_name={type_name:?} dimension={dimension}"
            ),
            SolveIssue::NotConverged => formatter.write_str("not_converged"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::DimensionValues;
    use crate::test_support::{id, message_context, sketch};

    #[test]
    fn a_redundant_constraint_the_sketch_does_not_have_is_named_by_its_type() {
        let sketch = sketch(Vec::new(), Vec::new());
        let values = DimensionValues::new();
        let issue = SolveIssue::Redundant {
            constraint: id("c-9"),
            type_name: "point_on_object",
            dimension: false,
        };

        assert_eq!(
            issue.render(&message_context(&sketch, &values)),
            "In 'Sketch 1', Point on object is not needed: the other constraints already hold it."
        );
    }

    #[test]
    fn detail_of_a_single_conflict_names_the_constraint() {
        let issue = SolveIssue::Conflicting {
            constraints: vec![id("c-1")],
        };

        assert_eq!(issue.to_string(), "conflicting constraints=c-1");
    }

    #[test]
    fn a_conflict_without_constraints_still_reads_as_a_sentence() {
        let sketch = sketch(Vec::new(), Vec::new());
        let values = DimensionValues::new();
        let context = message_context(&sketch, &values);
        let issue = SolveIssue::Conflicting {
            constraints: Vec::new(),
        };

        assert_eq!(
            issue.render(&context),
            "In 'Sketch 1', the constraints cannot all hold."
        );
        assert_eq!(issue.fixes(&sketch), Vec::new());
    }
}
