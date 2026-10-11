//! Built-in functions and their arities (expressions.md section 3.9).

use std::fmt;

/// Built-in functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Function {
    /// `abs`
    Abs,
    /// `sqrt`
    Sqrt,
    /// `sin`
    Sin,
    /// `cos`
    Cos,
    /// `tan`
    Tan,
    /// `asin`
    Asin,
    /// `acos`
    Acos,
    /// `atan`
    Atan,
    /// `atan2`
    Atan2,
    /// `min`
    Min,
    /// `max`
    Max,
    /// `floor`
    Floor,
    /// `ceil`
    Ceil,
    /// `round`
    Round,
}

impl Function {
    /// Every function, in declaration order.
    pub const ALL: [Function; 14] = [
        Function::Abs,
        Function::Sqrt,
        Function::Sin,
        Function::Cos,
        Function::Tan,
        Function::Asin,
        Function::Acos,
        Function::Atan,
        Function::Atan2,
        Function::Min,
        Function::Max,
        Function::Floor,
        Function::Ceil,
        Function::Round,
    ];

    /// The function's name as written in expressions.
    pub fn name(self) -> &'static str {
        match self {
            Function::Abs => "abs",
            Function::Sqrt => "sqrt",
            Function::Sin => "sin",
            Function::Cos => "cos",
            Function::Tan => "tan",
            Function::Asin => "asin",
            Function::Acos => "acos",
            Function::Atan => "atan",
            Function::Atan2 => "atan2",
            Function::Min => "min",
            Function::Max => "max",
            Function::Floor => "floor",
            Function::Ceil => "ceil",
            Function::Round => "round",
        }
    }

    /// The function for a name.
    pub fn from_name(name: &str) -> Option<Function> {
        Function::ALL
            .into_iter()
            .find(|function| function.name() == name)
    }

    /// How many arguments it takes.
    pub fn arity(self) -> Arity {
        match self {
            Function::Atan2 => Arity::Exactly(2),
            Function::Min | Function::Max => Arity::AtLeast(2),
            Function::Abs
            | Function::Sqrt
            | Function::Sin
            | Function::Cos
            | Function::Tan
            | Function::Asin
            | Function::Acos
            | Function::Atan
            | Function::Floor
            | Function::Ceil
            | Function::Round => Arity::Exactly(1),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Number of arguments a function takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arity {
    /// Exactly this many.
    Exactly(usize),
    /// This many or more.
    AtLeast(usize),
}

impl Arity {
    /// True when a call with `count` arguments matches.
    pub(crate) fn accepts(self, count: usize) -> bool {
        match self {
            Arity::Exactly(expected) => count == expected,
            Arity::AtLeast(minimum) => count >= minimum,
        }
    }
}

impl fmt::Display for Arity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arity::Exactly(1) => formatter.write_str("1 argument"),
            Arity::Exactly(count) => write!(formatter, "{count} arguments"),
            Arity::AtLeast(1) => formatter.write_str("at least 1 argument"),
            Arity::AtLeast(count) => write!(formatter, "at least {count} arguments"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for function in Function::ALL {
            assert_eq!(Function::from_name(function.name()), Some(function));
        }
        assert_eq!(Function::from_name("log"), None);
    }

    #[test]
    fn functions_display_their_name() {
        assert_eq!(Function::Atan2.to_string(), "atan2");
    }

    #[test]
    fn arity_text() {
        assert_eq!(Arity::Exactly(2).to_string(), "2 arguments");
        assert_eq!(Arity::AtLeast(1).to_string(), "at least 1 argument");
    }

    #[test]
    fn arity_accepts_counts() {
        assert!(Arity::Exactly(1).accepts(1));
        assert!(!Arity::Exactly(1).accepts(2));
        assert!(Arity::AtLeast(2).accepts(5));
        assert!(!Arity::AtLeast(2).accepts(1));
    }

    #[test]
    fn atan2_takes_two_arguments() {
        assert_eq!(Function::Atan2.arity(), Arity::Exactly(2));
        assert_eq!(Function::Round.arity(), Arity::Exactly(1));
    }
}
