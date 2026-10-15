//! Python code generation: calls, values and the script names hooks look up (features.md sections 2.10 and 3.11).

use std::collections::BTreeMap;
use std::fmt::{Display, Write as _};

use partsmith_model::canonical::format_number;
use partsmith_model::{Expr, Id, Units};
use partsmith_naming::Reference;
use partsmith_sketch::{OriginPlane, RegionRef};
use serde_json::{Number, Value};

use crate::params::{AxisRef, BodyOp, OriginAxis, PlaneRef};

/// Longest statement, indentation included, written on one line (python-api.md section 3.8 rule 11).
const MAX_LINE_LENGTH: usize = 88;

/// Extra indentation of each argument of a wrapped call.
const ARGUMENT_INDENT: usize = 4;

/// Most significant digits of a number written bare (python-api.md section 3.8 rule 8).
const MAX_SIGNIFICANT_DIGITS: usize = 15;

/// Smallest magnitude of a fractional number written bare; Python writes smaller ones with an exponent.
const MIN_FRACTIONAL_MAGNITUDE: f64 = 1e-4;

/// Python keywords (`keyword.kwlist`); soft keywords such as `type` and `match` are identifiers.
const PYTHON_KEYWORDS: [&str; 35] = [
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// Expression units with their Python names (python-api.md section 3.8 rule 8).
const PYTHON_UNITS: [(&str, &str); 6] = [
    ("mm", "mm"),
    ("cm", "cm"),
    ("m", "meter"),
    ("in", "inch"),
    ("deg", "deg"),
    ("rad", "rad"),
];

/// One generated Python call with positional and keyword arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonCall {
    /// Function name: a dedicated function of python-api.md section 2.7, a helper such as "Ref", or "feature" for the generic form.
    pub function: String,
    /// Positional arguments in order.
    pub positional: Vec<PythonValue>,
    /// Keyword arguments whose names are Python identifiers, in order.
    pub keywords: Vec<(String, PythonValue)>,
    /// Arguments whose names are not identifiers or are Python keywords, passed as a trailing `**{...}`.
    pub extra: Vec<(String, PythonValue)>,
}

impl PythonCall {
    /// A call of `function` with no arguments yet.
    pub fn new(function: impl Into<String>) -> PythonCall {
        PythonCall {
            function: function.into(),
            positional: Vec::new(),
            keywords: Vec::new(),
            extra: Vec::new(),
        }
    }

    /// Renders the statement at `indent` spaces (section 3.11). No trailing newline.
    ///
    /// A statement of at most 88 characters is one line; a longer one puts each argument on its
    /// own line, 4 spaces deeper and followed by a comma, and the closing parenthesis on its own
    /// line at the statement's indentation.
    pub fn render(&self, indent: usize) -> String {
        let prefix = " ".repeat(indent);
        let arguments = self.arguments();
        let line = format!("{prefix}{}({})", self.function, arguments.join(", "));
        if arguments.is_empty() || line.chars().count() <= MAX_LINE_LENGTH {
            return line;
        }
        let argument_prefix = " ".repeat(indent + ARGUMENT_INDENT);
        let mut text = format!("{prefix}{}(\n", self.function);
        for argument in &arguments {
            text.push_str(&argument_prefix);
            text.push_str(argument);
            text.push_str(",\n");
        }
        text.push_str(&prefix);
        text.push(')');
        text
    }

    /// Each argument as text: positional, then keywords, then the `**{...}` of `extra`.
    fn arguments(&self) -> Vec<String> {
        let mut arguments: Vec<String> = self.positional.iter().map(PythonValue::render).collect();
        arguments.extend(
            self.keywords
                .iter()
                .map(|(name, value)| format!("{name}={}", value.render())),
        );
        if !self.extra.is_empty() {
            arguments.push(format!("**{}", render_dict(&self.extra)));
        }
        arguments
    }
}

/// A Python value in generated code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PythonValue {
    /// `None`.
    None,
    /// `True` or `False`.
    Bool(bool),
    /// A number literal, in canonical JSON number form.
    Number(String),
    /// A string literal.
    Str(String),
    /// A list literal.
    List(Vec<PythonValue>),
    /// A dict literal with string keys, in order.
    Dict(Vec<(String, PythonValue)>),
    /// Text written verbatim: a variable, an attribute path or a quantity literal, for example `block`, `Axis.Z`, `line_1.start`, `10*mm`.
    Name(String),
    /// A nested call, always written on one line, for example `Ref("q", "fp", part=block)` or `Region("k", "fp")`.
    Call(Box<PythonCall>),
    /// A tuple literal, for example `(20*mm, 4*mm)`.
    Tuple(Vec<PythonValue>),
}

impl PythonValue {
    /// The literal of a JSON value, one to one (section 3.11).
    pub fn from_json(value: &Value) -> PythonValue {
        match value {
            Value::Null => PythonValue::None,
            Value::Bool(value) => PythonValue::Bool(*value),
            Value::Number(number) => PythonValue::Number(format_number(number)),
            Value::String(text) => PythonValue::Str(text.clone()),
            Value::Array(items) => {
                PythonValue::List(items.iter().map(PythonValue::from_json).collect())
            }
            Value::Object(entries) => PythonValue::Dict(
                entries
                    .iter()
                    .map(|(key, value)| (key.clone(), PythonValue::from_json(value)))
                    .collect(),
            ),
        }
    }

    /// An expression param (python-api.md section 3.8 rule 8): a bare number, a quantity literal, a parameter's variable, a `printer.*` name, or a string.
    pub fn expression(expr: &Expr, names: &dyn PythonNames) -> PythonValue {
        let source = expr.as_str();
        if is_short_number(source) {
            return PythonValue::Number(source.to_string());
        }
        if let Some(quantity) = quantity_literal(source) {
            return PythonValue::Name(quantity);
        }
        if let Some(variable) = names.parameter(source) {
            return PythonValue::Name(variable);
        }
        if is_printer_name(source) {
            return PythonValue::Name(source.to_string());
        }
        PythonValue::Str(source.to_string())
    }

    /// A raw length in millimeters (rule 9): the canonical number, written `<number>*mm` when the document units are not mm.
    pub fn length(value: f64, units: Units) -> PythonValue {
        let number = raw_number(value);
        match units {
            Units::Mm => PythonValue::Number(number),
            _ => PythonValue::Name(format!("{number}*mm")),
        }
    }

    /// A raw angle in radians (rule 9): `<number>*rad`.
    pub fn angle(value: f64) -> PythonValue {
        PythonValue::Name(format!("{}*rad", raw_number(value)))
    }

    /// A stored reference: `Ref("<q>", "<fp>")`, with `part=<part>` when a part variable is given, then `anchors=`
    /// and `merged=` when stored (rule 9, section 3.11).
    pub fn reference(reference: &Reference, part: Option<String>) -> PythonValue {
        PythonValue::Call(Box::new(reference_call(reference, part)))
    }

    /// A stored region: `Region("<key>", "<fp>")`, with `outline="<loop>"` when stored (section 3.11).
    pub fn region(region: &RegionRef) -> PythonValue {
        let mut call = PythonCall::new("Region");
        call.positional = vec![
            PythonValue::Str(region.key.to_string()),
            PythonValue::Str(region.fp.to_string()),
        ];
        if let Some(outline) = &region.outline {
            call.keywords
                .push(("outline".to_string(), PythonValue::Str(outline.to_string())));
        }
        PythonValue::Call(Box::new(call))
    }

    /// A plane (rule 9): `Plane.XY`, `Plane.XZ`, `Plane.YZ`, a datum plane's variable, or `Ref(..., part=...)`. None when a variable is missing.
    pub fn plane(plane: &PlaneRef, names: &dyn PythonNames) -> Option<PythonValue> {
        match plane {
            PlaneRef::Origin { plane } => Some(PythonValue::Name(
                match plane {
                    OriginPlane::Xy => "Plane.XY",
                    OriginPlane::Xz => "Plane.XZ",
                    OriginPlane::Yz => "Plane.YZ",
                }
                .to_string(),
            )),
            PlaneRef::Datum { feature } => names.feature(feature).map(PythonValue::Name),
            PlaneRef::Face { body, face } => names
                .body(body)
                .map(|part| PythonValue::reference(face, Some(part))),
        }
    }

    /// An axis (rule 9): `Axis.X`, `Axis.Y`, `Axis.Z`, a sketch line's variable, or `Ref(..., part=..., direction=(x, y, z))`
    /// with `direction` when stored. None when a variable is missing.
    pub fn axis(axis: &AxisRef, names: &dyn PythonNames) -> Option<PythonValue> {
        match axis {
            AxisRef::Origin { axis } => Some(PythonValue::Name(
                match axis {
                    OriginAxis::X => "Axis.X",
                    OriginAxis::Y => "Axis.Y",
                    OriginAxis::Z => "Axis.Z",
                }
                .to_string(),
            )),
            AxisRef::SketchLine { sketch, line } => {
                names.entity(sketch, line).map(PythonValue::Name)
            }
            AxisRef::Edge {
                body,
                edge: element,
                direction,
            }
            | AxisRef::Face {
                body,
                face: element,
                direction,
            } => names.body(body).map(|part| {
                let mut call = reference_call(element, Some(part));
                if let Some(direction) = direction {
                    let components = direction
                        .iter()
                        .map(|component| PythonValue::Number(raw_number(*component)))
                        .collect();
                    call.keywords
                        .push(("direction".to_string(), PythonValue::Tuple(components)));
                }
                PythonValue::Call(Box::new(call))
            }),
        }
    }

    /// Renders the value on one line.
    pub fn render(&self) -> String {
        match self {
            PythonValue::None => "None".to_string(),
            PythonValue::Bool(true) => "True".to_string(),
            PythonValue::Bool(false) => "False".to_string(),
            PythonValue::Number(text) | PythonValue::Name(text) => text.clone(),
            PythonValue::Str(text) => quote(text),
            PythonValue::List(items) => format!("[{}]", render_items(items)),
            PythonValue::Dict(entries) => render_dict(entries),
            PythonValue::Call(call) => {
                format!("{}({})", call.function, call.arguments().join(", "))
            }
            PythonValue::Tuple(items) => match items.as_slice() {
                [single] => format!("({},)", single.render()),
                _ => format!("({})", render_items(items)),
            },
        }
    }
}

/// The `Ref(...)` call of a stored reference: query and fingerprint, then `part`, `anchors` and `merged` when given,
/// keys and lists in stored order (rule 9).
fn reference_call(reference: &Reference, part: Option<String>) -> PythonCall {
    let mut call = PythonCall::new("Ref");
    call.positional = vec![
        PythonValue::Str(reference.query.to_string()),
        PythonValue::Str(reference.fingerprint.to_string()),
    ];
    if let Some(part) = part {
        call.keywords
            .push(("part".to_string(), PythonValue::Name(part)));
    }
    if !reference.anchors.is_empty() {
        call.keywords
            .push(("anchors".to_string(), name_lists(&reference.anchors.0)));
    }
    if !reference.merged.is_empty() {
        call.keywords
            .push(("merged".to_string(), name_lists(&reference.merged.0)));
    }
    call
}

/// A dict of string lists, from names to the names they list.
fn name_lists<K: Display, V: Display>(entries: &BTreeMap<K, Vec<V>>) -> PythonValue {
    PythonValue::Dict(
        entries
            .iter()
            .map(|(key, names)| {
                let names = names
                    .iter()
                    .map(|name| PythonValue::Str(name.to_string()))
                    .collect();
                (key.to_string(), PythonValue::List(names))
            })
            .collect(),
    )
}

/// Script context the engine's script generator gives each `Feature::python` hook (python-api.md section 3.8).
pub trait PythonNames {
    /// The document units.
    fn units(&self) -> Units;
    /// The variable of a body's part. None when the document has no such body.
    fn body(&self, body: &Id) -> Option<String>;
    /// The variable of a feature the call refers to (a sketch or a datum plane). None when the document has no such feature.
    fn feature(&self, feature: &Id) -> Option<String>;
    /// The variable of a sketch entity (a point, or a line used as an axis). None when the sketch has no such entity.
    fn entity(&self, sketch: &Id, entity: &Id) -> Option<String>;
    /// The variable of the parameter named `name`. None when no parameter has that name.
    fn parameter(&self, name: &str) -> Option<String>;
    /// Every region of a sketch in solved order, as `RegionRef`s. None when the sketch did not solve.
    fn regions(&self, sketch: &Id) -> Option<Vec<RegionRef>>;
    /// The op a solid feature function uses for `body` when `op` is omitted (python-api.md section 3.3): New when no earlier feature in the timeline targets the body, else Join.
    fn default_op(&self, body: &Id) -> BodyOp;
}

/// True for a keyword argument name: an identifier that is not a Python keyword.
pub(crate) fn is_keyword_name(name: &str) -> bool {
    is_identifier(name) && !PYTHON_KEYWORDS.contains(&name)
}

/// True for an ASCII identifier `[A-Za-z_][A-Za-z0-9_]*`.
fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// A double-quoted string literal with `\`, `"`, newline, carriage return and tab escaped, and
/// other characters below U+0020 and U+007F written `\xNN` (section 3.11).
fn quote(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            control if control < '\u{20}' || control == '\u{7f}' => {
                write!(literal, "\\x{:02x}", u32::from(control))
                    .expect("writing to a String never fails");
            }
            other => literal.push(other),
        }
    }
    literal.push('"');
    literal
}

/// Items separated by `", "`.
fn render_items(items: &[PythonValue]) -> String {
    items
        .iter()
        .map(PythonValue::render)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A dict literal `{"key": value, ...}`.
fn render_dict(entries: &[(String, PythonValue)]) -> String {
    let entries: Vec<String> = entries
        .iter()
        .map(|(key, value)| format!("{}: {}", quote(key), value.render()))
        .collect();
    format!("{{{}}}", entries.join(", "))
}

/// The canonical JSON form of a raw number. Non-finite values, which validated params never
/// hold, are written as the Python expressions that produce them.
fn raw_number(value: f64) -> String {
    match Number::from_f64(value) {
        Some(number) => format_number(&number),
        None if value.is_nan() => "float(\"nan\")".to_string(),
        None if value > 0.0 => "float(\"inf\")".to_string(),
        None => "-float(\"inf\")".to_string(),
    }
}

/// True for a short canonical number (python-api.md section 3.8 rule 8): it matches
/// `-?(0|[1-9][0-9]*)(\.[0-9]*[1-9])?`, is not `-0`, has at most 15 significant digits, and, with a
/// fractional part, a magnitude of at least 0.0001. These are the texts Python writes back unchanged.
fn is_short_number(text: &str) -> bool {
    let unsigned = text.strip_prefix('-').unwrap_or(text);
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    let integer_ok = integer == "0"
        || (integer.starts_with(|character: char| matches!(character, '1'..='9'))
            && integer.bytes().all(|byte| byte.is_ascii_digit()));
    let fraction_ok = fraction.is_none_or(|fraction| {
        fraction.ends_with(|character: char| matches!(character, '1'..='9'))
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
    });
    if !integer_ok || !fraction_ok || text == "-0" {
        return false;
    }
    let digits: String = unsigned.chars().filter(char::is_ascii_digit).collect();
    let significant = digits.trim_start_matches('0').len();
    let small_fraction = fraction.is_some()
        && unsigned
            .parse::<f64>()
            .is_ok_and(|value| value < MIN_FRACTIONAL_MAGNITUDE);
    significant <= MAX_SIGNIFICANT_DIGITS && !small_fraction
}

/// `<number>*<unit>` for a source `<short canonical number> <unit>` with exactly one space and a
/// unit of `mm`, `cm`, `m`, `in`, `deg` or `rad` (rule 8), with the unit's Python name.
fn quantity_literal(source: &str) -> Option<String> {
    let (number, unit) = source.split_once(' ')?;
    let python_unit = PYTHON_UNITS
        .iter()
        .find(|(expression_unit, _)| *expression_unit == unit)
        .map(|(_, python_unit)| python_unit)?;
    is_short_number(number).then(|| format!("{number}*{python_unit}"))
}

/// True for a source matching `printer(\.[A-Za-z_][A-Za-z0-9_]*)+`.
fn is_printer_name(source: &str) -> bool {
    source
        .strip_prefix("printer.")
        .is_some_and(|path| path.split('.').all(is_identifier))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoNames;

    impl PythonNames for NoNames {
        fn units(&self) -> Units {
            Units::Mm
        }

        fn body(&self, body: &Id) -> Option<String> {
            (body.as_str() == "b-1").then(|| "block".to_string())
        }

        fn feature(&self, feature: &Id) -> Option<String> {
            (feature.as_str() == "f-dp1").then(|| "plane_1".to_string())
        }

        fn entity(&self, _sketch: &Id, entity: &Id) -> Option<String> {
            (entity.as_str() == "e-9").then(|| "line_1".to_string())
        }

        fn parameter(&self, _name: &str) -> Option<String> {
            None
        }

        fn regions(&self, _sketch: &Id) -> Option<Vec<RegionRef>> {
            None
        }

        fn default_op(&self, _body: &Id) -> BodyOp {
            BodyOp::Join
        }
    }

    fn id(text: &str) -> Id {
        Id::new(text).expect("test ids are valid")
    }

    fn face() -> Reference {
        serde_json::from_value(serde_json::json!({"q": "f-box1/top", "fp": "plane|n=0,0,1"}))
            .expect("the reference is valid")
    }

    #[test]
    fn short_numbers() {
        for text in ["0", "40", "-3", "2.5", "0.0001", "-0.5", "123456789012345"] {
            assert!(is_short_number(text), "{text} is short");
        }
    }

    #[test]
    fn numbers_python_would_write_differently_are_not_short() {
        for text in [
            "-0",
            "00",
            "01",
            "2.50",
            "2.",
            ".5",
            "1e5",
            "0.00001",
            "1234567890123456",
            "+1",
            "",
            "-",
            "1.0",
        ] {
            assert!(!is_short_number(text), "{text} is not short");
        }
    }

    #[test]
    fn quantities_need_one_space_and_a_known_unit() {
        assert_eq!(quantity_literal("1.5 in").as_deref(), Some("1.5*inch"));
        assert_eq!(quantity_literal("2 m").as_deref(), Some("2*meter"));
        assert_eq!(quantity_literal("2  mm"), None);
        assert_eq!(quantity_literal("2 ft"), None);
        assert_eq!(quantity_literal("2.0 mm"), None);
    }

    #[test]
    fn printer_names() {
        assert!(is_printer_name("printer.fit.slip"));
        assert!(is_printer_name("printer._x"));
        assert!(!is_printer_name("printer"));
        assert!(!is_printer_name("printer."));
        assert!(!is_printer_name("printer.fit.1"));
        assert!(!is_printer_name("printer.fit slip"));
    }

    #[test]
    fn unknown_parameter_names_are_strings() {
        assert_eq!(
            PythonValue::expression(&Expr::new("height"), &NoNames),
            PythonValue::Str("height".to_string())
        );
    }

    #[test]
    fn keyword_names() {
        assert!(is_keyword_name("teeth"));
        assert!(is_keyword_name("match"));
        assert!(is_keyword_name("_private"));
        assert!(!is_keyword_name("class"));
        assert!(!is_keyword_name("2nd"));
        assert!(!is_keyword_name("tooth-count"));
        assert!(!is_keyword_name("zähne"));
        assert!(!is_keyword_name(""));
    }

    #[test]
    fn empty_collections() {
        assert_eq!(PythonValue::List(Vec::new()).render(), "[]");
        assert_eq!(PythonValue::Dict(Vec::new()).render(), "{}");
        assert_eq!(PythonValue::Tuple(Vec::new()).render(), "()");
    }

    #[test]
    fn call_without_arguments_is_never_wrapped() {
        let call = PythonCall::new("a".repeat(100));

        assert_eq!(call.render(0), format!("{}()", "a".repeat(100)));
    }

    #[test]
    fn raw_lengths_in_other_units_are_millimeter_literals() {
        assert_eq!(PythonValue::length(10.0, Units::Cm).render(), "10*mm");
        assert_eq!(PythonValue::length(-0.0, Units::Mm).render(), "0");
    }

    #[test]
    fn non_finite_raw_numbers_are_python_expressions() {
        assert_eq!(PythonValue::angle(f64::NAN).render(), "float(\"nan\")*rad");
        assert_eq!(
            PythonValue::length(f64::NEG_INFINITY, Units::Mm).render(),
            "-float(\"inf\")"
        );
    }

    #[test]
    fn origin_planes_and_axes() {
        let plane = PlaneRef::Origin {
            plane: OriginPlane::Yz,
        };
        let axis = AxisRef::Origin {
            axis: OriginAxis::Z,
        };

        assert_eq!(
            PythonValue::plane(&plane, &NoNames).map(|value| value.render()),
            Some("Plane.YZ".to_string())
        );
        assert_eq!(
            PythonValue::axis(&axis, &NoNames).map(|value| value.render()),
            Some("Axis.Z".to_string())
        );
    }

    #[test]
    fn datum_planes_and_sketch_lines_use_their_variables() {
        let datum = PlaneRef::Datum {
            feature: id("f-dp1"),
        };
        let missing = PlaneRef::Datum {
            feature: id("f-dp9"),
        };
        let line = AxisRef::SketchLine {
            sketch: id("f-sk1"),
            line: id("e-9"),
        };

        assert_eq!(
            PythonValue::plane(&datum, &NoNames),
            Some(PythonValue::Name("plane_1".to_string()))
        );
        assert_eq!(PythonValue::plane(&missing, &NoNames), None);
        assert_eq!(
            PythonValue::axis(&line, &NoNames),
            Some(PythonValue::Name("line_1".to_string()))
        );
    }

    #[test]
    fn face_planes_and_axes_carry_their_part() {
        let plane = PlaneRef::Face {
            body: id("b-1"),
            face: face(),
        };
        let axis = AxisRef::Face {
            body: id("b-2"),
            face: face(),
            direction: None,
        };

        assert_eq!(
            PythonValue::plane(&plane, &NoNames).map(|value| value.render()),
            Some(r#"Ref("f-box1/top", "plane|n=0,0,1", part=block)"#.to_string())
        );
        assert_eq!(PythonValue::axis(&axis, &NoNames), None);
    }

    #[test]
    fn references_without_a_part() {
        assert_eq!(
            PythonValue::reference(&face(), None).render(),
            r#"Ref("f-box1/top", "plane|n=0,0,1")"#
        );
    }

    #[test]
    fn regions_write_their_key_and_fingerprint() {
        let region: RegionRef =
            serde_json::from_value(serde_json::json!({"key": "e-1,~e-2", "fp": "c=1,2|a=3"}))
                .expect("the region ref is valid");

        assert_eq!(
            PythonValue::region(&region).render(),
            r#"Region("e-1,~e-2", "c=1,2|a=3")"#
        );
    }

    #[test]
    fn nested_calls_stay_on_one_line_when_the_statement_wraps() {
        let mut call = PythonCall::new("extrude");
        call.positional = vec![PythonValue::Name("sketch_1".to_string())];
        call.keywords = vec![(
            "regions".to_string(),
            PythonValue::List(vec![
                PythonValue::region(
                    &serde_json::from_value(
                        serde_json::json!({"key": "e-1,e-2,e-3,e-4", "fp": "c=40,25|a=4000"}),
                    )
                    .expect("valid"),
                ),
                PythonValue::region(
                    &serde_json::from_value(
                        serde_json::json!({"key": "e-5", "fp": "c=40,25|a=314.159265"}),
                    )
                    .expect("valid"),
                ),
            ]),
        )];

        assert_eq!(
            call.render(0),
            [
                "extrude(",
                "    sketch_1,",
                r#"    regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000"), Region("e-5", "c=40,25|a=314.159265")],"#,
                ")",
            ]
            .join("\n")
        );
    }
}
