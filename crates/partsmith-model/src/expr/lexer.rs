//! Splits expression source text into tokens (expressions.md section 3.2).

use super::{ExprError, ExprErrorKind, Span};

/// The degree sign, U+00B0.
const DEGREE_SIGN: char = '\u{b0}';

/// What a token is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TokenKind {
    /// A number literal and its value.
    Number(f64),
    /// An identifier, or a dotted name when `dotted` is true.
    Name {
        /// True when the name has at least one `.`.
        dotted: bool,
    },
    /// The degree sign.
    DegreeSign,
    /// `**`, which is always an error.
    PowerError,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `^`
    Caret,
    /// `(`
    OpenParenthesis,
    /// `)`
    CloseParenthesis,
    /// `,`
    Comma,
}

/// One token and where it is in the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Token {
    /// What the token is.
    pub(super) kind: TokenKind,
    /// Its byte range in the source.
    pub(super) span: Span,
}

/// True for the whitespace an expression may contain between tokens: space, tab, LF and CR.
pub(super) fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// Splits `source` into tokens, or returns the first character no token can start with.
pub(super) fn tokenize(source: &str) -> Result<Vec<Token>, ExprError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut position = 0;
    while let Some(&byte) = bytes.get(position) {
        let start = position;
        let kind = match byte {
            byte if is_whitespace(byte) => {
                position += 1;
                continue;
            }
            b'0'..=b'9' => {
                position = number_end(bytes, position);
                TokenKind::Number(number_value(&source[start..position]))
            }
            b'.' if bytes.get(position + 1).is_some_and(u8::is_ascii_digit) => {
                position = number_end(bytes, position);
                TokenKind::Number(number_value(&source[start..position]))
            }
            byte if is_identifier_start(byte) => {
                let (end, dotted) = name_end(bytes, position);
                position = end;
                TokenKind::Name { dotted }
            }
            b'*' if bytes.get(position + 1) == Some(&b'*') => {
                position += 2;
                TokenKind::PowerError
            }
            byte => match symbol(byte) {
                Some(kind) => {
                    position += 1;
                    kind
                }
                None => {
                    let character = source[position..]
                        .chars()
                        .next()
                        .expect("the position is a character boundary inside the source");
                    position += character.len_utf8();
                    if character != DEGREE_SIGN {
                        return Err(ExprError::new(
                            ExprErrorKind::UnexpectedCharacter { character },
                            Span::new(start, position),
                        ));
                    }
                    TokenKind::DegreeSign
                }
            },
        };
        tokens.push(Token {
            kind,
            span: Span::new(start, position),
        });
    }
    Ok(tokens)
}

/// The token kind of a one-character symbol, or `None` when `byte` is not one.
fn symbol(byte: u8) -> Option<TokenKind> {
    let kind = match byte {
        b'+' => TokenKind::Plus,
        b'-' => TokenKind::Minus,
        b'*' => TokenKind::Star,
        b'/' => TokenKind::Slash,
        b'^' => TokenKind::Caret,
        b'(' => TokenKind::OpenParenthesis,
        b')' => TokenKind::CloseParenthesis,
        b',' => TokenKind::Comma,
        _ => return None,
    };
    Some(kind)
}

/// True for the bytes an identifier can start with: ASCII letters and `_`.
fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

/// True for the bytes an identifier can continue with: ASCII letters, digits and `_`.
fn is_identifier_part(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Position after the digits starting at `position`.
fn digits_end(bytes: &[u8], mut position: usize) -> usize {
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    position
}

/// Position after the number literal starting at `start`: integer digits, an optional `.` with
/// digits, then an exponent only when `e` or `E` is followed by a digit or by a sign and a digit.
fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut end = digits_end(bytes, start);
    if bytes.get(end) == Some(&b'.') {
        end = digits_end(bytes, end + 1);
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        if bytes.get(exponent).is_some_and(u8::is_ascii_digit) {
            end = digits_end(bytes, exponent);
        }
    }
    end
}

/// The correctly rounded value of a number literal, infinite when it is too large.
fn number_value(text: &str) -> f64 {
    text.parse()
        .expect("number literals match the float syntax of str::parse")
}

/// Position after the name starting at `start`, and whether it is dotted: an identifier extended
/// by `.identifier` while a `.` is immediately followed by a letter or `_`.
fn name_end(bytes: &[u8], start: usize) -> (usize, bool) {
    let identifier_end = |mut position: usize| {
        while bytes
            .get(position)
            .is_some_and(|byte| is_identifier_part(*byte))
        {
            position += 1;
        }
        position
    };
    let mut end = identifier_end(start);
    let mut dotted = false;
    while bytes.get(end) == Some(&b'.')
        && bytes
            .get(end + 1)
            .is_some_and(|byte| is_identifier_start(*byte))
    {
        end = identifier_end(end + 1);
        dotted = true;
    }
    (end, dotted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .expect("the source lexes")
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    fn texts(source: &str) -> Vec<&str> {
        tokenize(source)
            .expect("the source lexes")
            .into_iter()
            .map(|token| &source[token.span.start..token.span.end])
            .collect()
    }

    #[test]
    fn whitespace_is_skipped() {
        assert_eq!(texts(" \t1\n+\r2 "), ["1", "+", "2"]);
    }

    #[test]
    fn number_forms() {
        assert_eq!(
            kinds("5. .5 2.e3 1E-3 2.5e+1"),
            [
                TokenKind::Number(5.0),
                TokenKind::Number(0.5),
                TokenKind::Number(2000.0),
                TokenKind::Number(0.001),
                TokenKind::Number(25.0),
            ]
        );
    }

    #[test]
    fn an_e_without_digits_ends_the_number() {
        assert_eq!(texts("2em"), ["2", "em"]);
        assert_eq!(texts("2e+"), ["2", "e", "+"]);
    }

    #[test]
    fn huge_literals_are_infinite() {
        assert_eq!(kinds("1e400"), [TokenKind::Number(f64::INFINITY)]);
    }

    #[test]
    fn a_second_dot_starts_a_new_number() {
        assert_eq!(texts("1.2.3"), ["1.2", ".3"]);
    }

    #[test]
    fn dotted_names_are_one_token() {
        assert_eq!(
            kinds("printer.fit.slip wall"),
            [
                TokenKind::Name { dotted: true },
                TokenKind::Name { dotted: false }
            ]
        );
        assert_eq!(texts("printer.fit.in"), ["printer.fit.in"]);
    }

    #[test]
    fn a_dot_before_a_digit_ends_a_name() {
        assert_eq!(texts("a.5"), ["a", ".5"]);
    }

    #[test]
    fn a_trailing_dot_is_an_unexpected_character() {
        assert_eq!(
            tokenize("wall."),
            Err(ExprError::new(
                ExprErrorKind::UnexpectedCharacter { character: '.' },
                Span::new(4, 5)
            ))
        );
    }

    #[test]
    fn double_star_is_one_token() {
        assert_eq!(
            kinds("2***3"),
            [
                TokenKind::Number(2.0),
                TokenKind::PowerError,
                TokenKind::Star,
                TokenKind::Number(3.0)
            ]
        );
    }

    #[test]
    fn symbols() {
        assert_eq!(
            kinds("+-*/^(),°"),
            [
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Caret,
                TokenKind::OpenParenthesis,
                TokenKind::CloseParenthesis,
                TokenKind::Comma,
                TokenKind::DegreeSign
            ]
        );
    }

    #[test]
    fn non_ascii_characters_span_their_bytes() {
        assert_eq!(
            tokenize("1 + 中"),
            Err(ExprError::new(
                ExprErrorKind::UnexpectedCharacter { character: '中' },
                Span::new(4, 7)
            ))
        );
    }
}
