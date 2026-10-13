# Expressions (`partsmith-model`)

Behavior spec for expressions: the `Expr` type, its grammar, units, dimension checking, implicit units by context, functions, names resolved through an evaluation context, the parameter graph, references, renaming and display text. It is part of the `partsmith-model` crate; see [model.md](model.md) for the document, parameter records and parameter name rules, and [file-format.md](file-format.md) for how expressions are stored.

Contracts come from [architecture.md](../design/architecture.md) ("Expressions" under "Document model", and "Conventions shared by all crates"). Interpretations are listed under [Open issues](#7-open-issues).

## 1. Purpose and scope

Every numeric field in Partsmith accepts an expression: `2 mm`, `30deg`, `2 * wall + clearance`, `printer.fit.slip`, `sqrt(width^2 + depth^2)`. An `Expr` stores the source text exactly as typed. Evaluation is dimension-checked and returns a `Quantity` in internal units (millimeters for lengths, radians for angles). Bare numbers take their unit from context: document units for lengths, degrees for angles.

In this milestone (M1):

- `Expr` as a newtype over the source string, with parse, evaluate, references and rename.
- The full grammar: numbers with optional units, unit names as values, parameter names, dotted names, `+ - * / ^`, parentheses, unary minus, the constant `pi`, and the functions `abs sqrt sin cos tan asin acos atan atan2 min max floor ceil round`.
- Units `um mm cm m in ft deg rad` and the degree sign.
- `Dimension` (integer exponents of length and angle) and `Quantity`.
- Plain-number promotion (implicit units by context) and the expected dimension of a field.
- The `EvaluationContext` trait for dotted names such as `printer.fit.slip`.
- Parameter graph evaluation with cycle detection and error propagation.
- A public parse tree (`ParsedExpr`) for the Python code generator and other tools.
- Display text for quantities.
- A suggestion for an unknown parameter name that is close to exactly one parameter ("Did you mean 'height'?"), with the corrected expression as the fix (section 3.18).
- Bare numbers: `Expr::is_bare_number` and `Expr::with_unit`, which the document units change that keeps sizes (model.md section 3.12) and the features' bare-number sentence use (section 3.17).

Explicitly later (not in M1):

- Comparison operators, booleans and conditionals (`if(a > b, x, y)`), needed for configurations.
- More functions (`log`, `exp`, `mod`, `hypot`, `sign`, rounding to a step).
- Mass, time and other dimensions (mass properties use their own types).
- Values measured from geometry (`f-ext1.height`, distances between elements).
- Cross-file references to parameters of linked components.
- Keeping sizes for numbers inside larger expressions when the document units change (model.md section 3.12 rewrites bare numbers only).
- Suggestions for unknown dotted names, units and functions.
- Localized decimal commas (the comma is reserved for argument lists; see `DecimalComma`).

## 2. Public API

All items are re-exported from the crate root.

```rust
/// Longest expression source, in Unicode scalar values.
pub const MAX_EXPRESSION_LENGTH: usize = 1000;
/// Deepest parse tree and deepest parenthesis nesting allowed.
pub const MAX_EXPRESSION_DEPTH: usize = 64;
/// Largest edit distance at which an unknown name is close to a parameter name (section 3.18).
pub const SUGGESTION_DISTANCE: usize = 2;

/// Levenshtein distance between two texts, counted in Unicode scalar values, case-sensitive (section 3.18).
pub fn edit_distance(a: &str, b: &str) -> usize;

/// An expression, stored as its source text exactly as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Expr(String);

impl Expr {
    /// Wraps source text; never fails, and keeps the text unchanged (including surrounding whitespace).
    pub fn new(source: impl Into<String>) -> Expr;
    /// The source text.
    pub fn as_str(&self) -> &str;
    /// Parses the source into a tree (sections 3.1 to 3.5).
    pub fn parse(&self) -> Result<ParsedExpr, ExprError>;
    /// Parses and evaluates; `expected` is the field's dimension, or `None` to keep the result as is.
    pub fn evaluate(&self, scope: &Scope<'_>, expected: Option<Dimension>) -> Result<Quantity, ExprError>;
    /// Parameter names and dotted names used, each in first-occurrence order without repeats.
    pub fn references(&self) -> Result<References, ExprError>;
    /// Replaces every reference to parameter `from` with `to`, keeping all other text byte for byte.
    pub fn rename_parameter(&self, from: &str, to: &str) -> Result<Expr, RenameError>;
    /// True when the expression is a number without a unit, optionally negated (section 3.17).
    pub fn is_bare_number(&self) -> bool;
    /// For a bare number, the same text with `unit` written after the number; `None` otherwise (section 3.17).
    pub fn with_unit(&self, unit: Unit) -> Option<Expr>;
}
// Also implemented: From<&str>, From<String>, Display (the source text), and JsonSchema as
// {"type": "string"} under the definition name "Expr".

/// Byte range in the source text, `start` inclusive and `end` exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

/// A successfully parsed expression.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedExpr {
    /// The root node.
    pub root: ExprNode,
}

impl ParsedExpr {
    /// Evaluates the tree; same rules as `Expr::evaluate`.
    pub fn evaluate(&self, scope: &Scope<'_>, expected: Option<Dimension>) -> Result<Quantity, ExprError>;
    /// Same as `Expr::references`.
    pub fn references(&self) -> References;
}

/// One node of the parse tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ExprNode {
    /// What the node is.
    pub kind: ExprNodeKind,
    /// Source range, including the parentheses when the node was written inside them.
    pub span: Span,
}

/// Node kinds. Parentheses are not nodes; they only widen spans.
#[derive(Clone, Debug, PartialEq)]
pub enum ExprNodeKind {
    /// A number literal with an optional unit suffix; `value` is the number as written, not converted.
    Number { value: f64, unit: Option<Unit> },
    /// A unit name used as a value, such as `mm` in `2 * mm^2`; it means one of that unit.
    Unit(Unit),
    /// The constant `pi`.
    Pi,
    /// A reference to the parameter with this name.
    Parameter(String),
    /// A dotted name such as `printer.fit.slip`, resolved by the evaluation context.
    Value(String),
    /// Unary minus.
    Negate(Box<ExprNode>),
    /// A binary operation.
    Binary { operator: BinaryOperator, left: Box<ExprNode>, right: Box<ExprNode> },
    /// A function call.
    Call { function: Function, arguments: Vec<ExprNode> },
}

/// Binary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `^`
    Power,
}

/// Units an expression can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// `um`
    Micrometer,
    /// `mm`
    Millimeter,
    /// `cm`
    Centimeter,
    /// `m`
    Meter,
    /// `in`
    Inch,
    /// `ft`
    Foot,
    /// `deg` or the degree sign
    Degree,
    /// `rad`
    Radian,
}

impl Unit {
    /// Every unit, in declaration order.
    pub const ALL: [Unit; 8];
    /// The unit's name: "um", "mm", "cm", "m", "in", "ft", "deg" or "rad".
    pub fn symbol(self) -> &'static str;
    /// The unit for a name; also maps "°" (U+00B0) to `Degree`.
    pub fn from_symbol(symbol: &str) -> Option<Unit>;
    /// `Dimension::LENGTH` or `Dimension::ANGLE`.
    pub fn dimension(self) -> Dimension;
    /// Internal units (millimeters or radians) in one of this unit.
    pub fn factor(self) -> f64;
}

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
    pub const ALL: [Function; 14];
    /// The function's name as written in expressions.
    pub fn name(self) -> &'static str;
    /// The function for a name.
    pub fn from_name(name: &str) -> Option<Function>;
    /// How many arguments it takes.
    pub fn arity(self) -> Arity;
}
// Display writes the name.

/// Number of arguments a function takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arity {
    /// Exactly this many.
    Exactly(usize),
    /// This many or more.
    AtLeast(usize),
}
// Display: "1 argument", "2 arguments", "at least 2 arguments".

/// Physical dimension as integer exponents of length and angle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Dimension {
    /// Exponent of length.
    pub length: i8,
    /// Exponent of angle.
    pub angle: i8,
}

impl Dimension {
    /// A plain number (dimensionless).
    pub const NONE: Dimension = Dimension { length: 0, angle: 0 };
    /// Length.
    pub const LENGTH: Dimension = Dimension { length: 1, angle: 0 };
    /// Area.
    pub const AREA: Dimension = Dimension { length: 2, angle: 0 };
    /// Volume.
    pub const VOLUME: Dimension = Dimension { length: 3, angle: 0 };
    /// Angle.
    pub const ANGLE: Dimension = Dimension { length: 0, angle: 1 };
    /// Largest absolute exponent an evaluation may produce.
    pub const MAX_EXPONENT: i8 = 16;
    /// True for `NONE`.
    pub fn is_plain(self) -> bool;
    /// Words for messages, such as "a length" (section 3.7).
    pub fn describe(self) -> String;
    /// Display unit label, such as "mm^2" (section 3.16).
    pub fn unit_label(self, units: Units) -> String;
    /// Internal units in one display unit of this dimension (section 3.7).
    pub fn display_factor(self, units: Units) -> f64;
}

/// A value in internal units (millimeters and radians) with its dimension.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Quantity {
    /// The value in internal units.
    pub value: f64,
    /// Its dimension.
    pub dimension: Dimension,
}

impl Quantity {
    /// A quantity with this value and dimension.
    pub fn new(value: f64, dimension: Dimension) -> Quantity;
    /// A plain number.
    pub fn plain(value: f64) -> Quantity;
    /// A length in millimeters.
    pub fn length(millimeters: f64) -> Quantity;
    /// An angle in radians.
    pub fn angle(radians: f64) -> Quantity;
    /// Display text in document units, which parses back to the same quantity (section 3.16).
    pub fn display(&self, units: Units) -> String;
}

/// Resolves dotted names such as `printer.fit.slip`. Implemented by the engine (printer values) and by tests.
pub trait EvaluationContext: Send + Sync {
    /// The value of a dotted name in internal units, or `None` when the name is unknown.
    fn resolve(&self, name: &str) -> Option<Quantity>;
    /// Every name this context resolves, sorted, for autocomplete. Defaults to none.
    fn names(&self) -> Vec<String> { Vec::new() }
}

/// A context that resolves nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyContext;

impl EvaluationContext for EmptyContext { /* resolve returns None */ }

/// Map-backed context: `resolve` looks the name up; `names` returns the keys in order.
impl EvaluationContext for std::collections::BTreeMap<String, Quantity> { /* ... */ }

/// Evaluated parameters by name, in document order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterValues { /* private */ }

impl ParameterValues {
    /// The result for a parameter name.
    pub fn get(&self, name: &str) -> Option<&Result<Quantity, ExprError>>;
    /// Every (name, result) in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Result<Quantity, ExprError>)>;
    /// Number of entries.
    pub fn len(&self) -> usize;
    /// True when there are no entries.
    pub fn is_empty(&self) -> bool;
}

impl FromIterator<(String, Result<Quantity, ExprError>)> for ParameterValues {
    /// Keeps insertion order; when a name repeats, the first entry wins.
    fn from_iter<I: IntoIterator<Item = (String, Result<Quantity, ExprError>)>>(iter: I) -> ParameterValues;
}

/// Everything an evaluation can see.
#[derive(Clone, Copy)]
pub struct Scope<'a> {
    /// Document units: the unit of plain numbers promoted to lengths.
    pub units: Units,
    /// Evaluated parameters.
    pub parameters: &'a ParameterValues,
    /// Resolver for dotted names.
    pub context: &'a dyn EvaluationContext,
}

/// Names an expression uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct References {
    /// Parameter names, in first-occurrence order, without repeats.
    pub parameters: Vec<String>,
    /// Dotted names, in first-occurrence order, without repeats.
    pub values: Vec<String>,
}

/// An expression error with the source range it points at.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("{kind}")]
pub struct ExprError {
    /// What went wrong.
    pub kind: ExprErrorKind,
    /// Where (section 4 says which range each kind uses).
    pub span: Span,
}

impl ExprError {
    /// For `UnknownName` with a suggestion, `source` with every reference to the unknown name replaced by the
    /// suggested one (section 3.18); `None` for every other error. `source` is the expression that gave the error.
    pub fn corrected(&self, source: &Expr) -> Option<Expr>;
}

/// Operation named in a dimension mismatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MismatchOperation {
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `min(...)`
    Min,
    /// `max(...)`
    Max,
    /// `atan2(...)`
    Atan2,
}

/// Error from `Expr::rename_parameter`.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum RenameError {
    /// The expression does not parse.
    #[error("{0}")]
    Parse(ExprError),
    /// The new name is not a valid parameter name.
    #[error("{0}")]
    InvalidName(NameError),
}
```

`ExprErrorKind` is listed with every variant, its fields, its span and its message in section 4.

JSON examples. `Expr` is a string, `Dimension` an object with both exponents always present, `Quantity` an object with `value` in internal units:

```json
"2 * wall + clearance"
{"length": 1, "angle": 0}
{"length": 2, "angle": 0}
{"length": 0, "angle": 1}
{"value": 25.4, "dimension": {"length": 1, "angle": 0}}
{"value": 0.5235987755982988, "dimension": {"length": 0, "angle": 1}}
{"value": 4, "dimension": {"length": 0, "angle": 0}}
```

Expressions are only ever stored as JSON strings. A JSON number where an `Expr` is expected (such as `"expr": 2`) is a deserialization error, not a conversion.

## 3. Behavior

### 3.1 Source text and limits

- `Expr` keeps its text verbatim; nothing is trimmed or normalized, so saving an unchanged expression is byte-identical.
- Before lexing: if the text contains only whitespace (or nothing), the error is `Empty`. If it has more than `MAX_EXPRESSION_LENGTH` (1000) Unicode scalar values, the error is `TooLong`.
- No input string, of any length or content, makes parsing or evaluation panic or overflow the stack.
- Spans are byte offsets into the source text.

### 3.2 Lexical grammar

The whole source is split into tokens first; the first lexical error, scanning left to right, is reported before any syntax error.

```ebnf
whitespace  = " " | "\t" | "\n" | "\r" ;            (* skipped between tokens *)
digit       = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;
letter      = "A" | ... | "Z" | "a" | ... | "z" ;   (* ASCII only *)
number      = ( digit , { digit } , [ "." , { digit } ]
              | "." , digit , { digit } ) , [ exponent ] ;
exponent    = ( "e" | "E" ) , [ "+" | "-" ] , digit , { digit } ;
identifier  = ( letter | "_" ) , { letter | digit | "_" } ;
name        = identifier , { "." , identifier } ;  (* one token; no whitespace inside *)
degree_sign = "°" ;                                (* U+00B0 *)
power_error = "**" ;                               (* one token; always an error *)
symbol      = "+" | "-" | "*" | "/" | "^" | "(" | ")" | "," ;
```

Tokenizing rules, applied at each position after skipping whitespace:

1. A digit, or a `.` followed by a digit, starts a `number`. The integer digits are taken; then a `.` and any digits after it; then an exponent only if `e` or `E` is followed by a digit, or by `+` or `-` and then a digit. Otherwise the number ends before the `e`. So `2e3` is 2000, `2em` is the number `2` then the name `em`, and `5.` is 5. The literal's value is the correctly rounded `f64` of its text (as Rust's `str::parse::<f64>` computes); it may be infinite (`1e400`), which evaluation reports as `NotFinite`.
2. A letter or `_` starts a `name`: an identifier, extended by `.identifier` as long as a `.` is immediately followed by a letter or `_`. A `.` followed by anything else ends the name (and is then lexed on its own). A name with at least one `.` is a dotted name.
3. `°` is the degree sign. `**` is one `power_error` token. Each `symbol` character is its own token.
4. Any other character (including non-ASCII letters, `$`, `;`, `=`, `"`, `[`, and a `.` not followed by a digit at token start) is `UnexpectedCharacter`.

### 3.3 Syntax

```ebnf
expression = sum ;
sum        = product , { ( "+" | "-" ) , product } ;
product    = unary , { ( "*" | "/" ) , unary } ;
unary      = "-" , unary | power ;
power      = primary , [ "^" , unary ] ;
primary    = number , [ unit ]
           | function_name , "(" , [ expression , { "," , expression } ] , ")"
           | name
           | "(" , expression , ")" ;
unit       = unit_name | degree_sign ;
```

`unit_name` is an identifier token (no dot) whose text is one of `um mm cm m in ft deg rad`. `function_name` is an identifier token whose text is a function name (section 3.9).

How a `name` token in `primary` position is read, in this order:

1. Followed by `(`: a function call if the name is a function name; otherwise `UnknownFunction` (this includes dotted names and parameter names, as in `wall(2)`).
2. A function name not followed by `(`: `MissingCall`.
3. `pi`: the constant.
4. A unit name: the unit as a value (one of that unit).
5. A dotted name: `ExprNodeKind::Value`.
6. Otherwise: `ExprNodeKind::Parameter`.

After a `number`:

- An identifier that is a unit name, or the degree sign, is its unit suffix. Whitespace between them is allowed (`2 mm`, `2mm`, `30°`, `30 °`).
- Any other name token (identifier or dotted) is `UnknownUnit` (`2 inch`, `2 wall`). Juxtaposition is never multiplication.

Other parser rules:

- `power_error` anywhere is `PowerOperator`.
- A `,` is an argument separator only when the innermost open parenthesis is a function call's. Anywhere else it is `DecimalComma` (`2,5`, `(2,5)`).
- Where an operand is expected and the next token is `)`: `UnmatchedParenthesis` if no parenthesis is open, otherwise `UnexpectedToken`. A call with nothing between its parentheses (`sqrt()`) is a call with zero arguments.
- Where an operator, `)`, `,` or the end is expected and something else comes (`2 3`, `2 (3)`, `(1) 2`, `wall mm`): `UnexpectedToken`. A `)` there with no parenthesis open is `UnmatchedParenthesis` (`2 + 3)`).
- The degree sign is only a unit suffix; anywhere else it is `UnexpectedToken` (`30 * °`).
- At the end of the input: if an operand is expected, `UnexpectedEnd`; if a value is complete but a parenthesis is still open, `UnclosedParenthesis` pointing at the innermost unclosed `(`.
- A call's argument count is checked when its `)` is read (`ArgumentCount`).
- Depth: opening a parenthesis (grouping or call) that makes more than 64 parentheses open at once, or building a node whose tree depth exceeds 64, is `TooDeep`. Tree depth: a leaf (`Number`, `Unit`, `Pi`, `Parameter`, `Value`) is 1; `Negate` is 1 plus its operand's depth; `Binary` is 1 plus the deeper operand; `Call` is 1 plus its deepest argument (1 with no arguments). The parser reports `TooDeep` as soon as either limit is passed, scanning left to right.
- Spans: a number with a unit spans both; a call spans its name through its `)`; `Negate` spans the `-` through its operand; `Binary` spans its left operand's start through its right operand's end; a node written inside parentheses has its span widened to include them, once per pair.

Unknown parameter names and unknown dotted names are not parse errors: whether they exist depends on the scope, so they are reported by evaluation (and by `Document::validate` for parameter names).

### 3.4 Precedence and associativity

From loosest to tightest binding:

| Level | Operators | Associativity |
| --- | --- | --- |
| 1 | binary `+`, `-` | left |
| 2 | `*`, `/` | left |
| 3 | unary `-` | prefix, may repeat (`--2` is 2) |
| 4 | `^` | right; its right operand may start with unary minus |
| 5 | number with unit, call, name, parentheses | |

Consequences, each an acceptance test: `10 - 4 - 3` is 3; `64 / 4 / 2` is 8; `2 ^ 3 ^ 2` is 512; `-2 ^ 2` is -4; `2 ^ -1` is 0.5; `2 ^ -2 ^ 2` is 2^(-(2^2)) = 0.0625; `2 * -3` is -6. A number with its unit is one operand, so `2 mm ^ 2` is (2 mm)^2 = 4 mm^2 while `2 * mm ^ 2` is 2 mm^2, and `wall / 2 mm` is wall / (2 mm).

### 3.5 Names

- **Parameters** are referred to by their exact, case-sensitive name. Parameter names follow model.md section 3.3; they can never collide with `pi`, unit names or function names because those are reserved.
- **Dotted names** (`printer.fit.slip`, `printer.nozzle`) are passed whole, as their source text, to `EvaluationContext::resolve`. Their segments may be any identifiers, including reserved words (`printer.fit.in` is a valid dotted name). The print spec defines which `printer.*` names exist; the engine's context resolves them from the document's printer profile.
- `pi` is the plain number `std::f64::consts::PI`.
- A unit name used as a value is one of that unit: `mm` is 1 mm, `deg` is 1 degree.

### 3.6 Units

| Name | Dimension | Factor (internal units per unit) |
| --- | --- | --- |
| `um` | length | `0.001` mm |
| `mm` | length | `1` mm |
| `cm` | length | `10` mm |
| `m` | length | `1000` mm |
| `in` | length | `25.4` mm |
| `ft` | length | `304.8` mm |
| `deg`, `°` | angle | `PI / 180.0` rad |
| `rad` | angle | `1` rad |

A number with a unit evaluates to `value * factor` with the unit's dimension. Unit names are case-sensitive (`MM` is not a unit). Document units (`mm`, `cm`, `m`, `in`) map to `Unit::Millimeter`, `Centimeter`, `Meter` and `Inch`.

### 3.7 Dimensions and plain numbers

- A `Dimension` is a pair of integer exponents (length, angle). Internal units are millimeters and radians, so a `Quantity` of dimension (a, b) is in mm^a rad^b.
- A **plain number** is a quantity of dimension `NONE`. Bare literals, `pi`, ratios such as `wall / thickness`, results of `sin`, `cos` and `tan`, and parameters whose expressions have no units are all plain.
- **Promotion.** When an operation needs a plain number to have dimension D (D not `NONE`), the plain number is read in display units: its value is multiplied by `D.display_factor(units)` and it takes dimension D. This is how bare numbers take their unit from context.
- `display_factor` for D = (a, b) is `units.millimeters().powi(a) * (PI / 180.0).powi(b)`: document units for lengths and degrees for angles. For `NONE` it is 1. Example: with document units `in`, a plain `10` promoted to a length is 254 mm; promoted to an area it is 6451.6 mm^2.
- Promotion happens only where section 3.8, 3.9 or 3.10 says so: in `+` and `-`, in `min`, `max` and `atan2`, in the argument of `sin`, `cos` and `tan`, and against the field's expected dimension. Multiplication, division and powers never promote: in `2 * wall` the 2 stays a plain factor.
- A consequence worth knowing: because ratios are plain too, `wall / thickness + 1 mm` reads the ratio in document units. This is consistent with the rule and is documented for users.
- `Dimension::describe()`: `NONE` is "a plain number", `LENGTH` "a length", `AREA` "an area", `VOLUME` "a volume", `ANGLE` "an angle"; any other dimension is "a quantity in " followed by its internal unit label: `mm` with its length exponent, then `rad` with its angle exponent, each omitted when zero, exponent written as `^n` only when not 1, joined by `*` (for example "a quantity in mm^-1*rad").

### 3.8 Operators

Operands are evaluated left before right. `l` and `r` are the operand quantities.

| Operator | Dimension rule | Value | Errors, in check order |
| --- | --- | --- | --- |
| `-x` (unary) | same as `x` | `-x` | |
| `l + r`, `l - r` | unify (below) with operation `Add` or `Subtract` | sum or difference after promotion | `DimensionMismatch` |
| `l * r` | exponents add | product | `DimensionOverflow` |
| `l / r` | exponents subtract | quotient | `DivisionByZero` (when `r.value == 0`, including -0), `DimensionOverflow` |
| `l ^ r` | `r` must be plain; if `l` is plain the result is plain; otherwise each exponent of `l` is multiplied by `r.value` | `l.value.powf(r.value)` | `ExponentDimension` (span of `r`), `NegativeBase` (when `l.value < 0` and `r.value` is not an integer), `FractionalDimension` (a scaled exponent is more than `1e-9` from an integer), `DimensionOverflow` |

**Unify** (used by `+`, `-`, `min`, `max`, `atan2`) over operands in order:

1. Let D be the dimension of the first operand that is not plain. If every operand is plain, nothing changes and the result is plain.
2. If a later non-plain operand has a dimension other than D, report `DimensionMismatch { operation, left: D, right: that dimension }`.
3. Promote every plain operand to D.

`DimensionOverflow` is reported when any resulting exponent is outside `-16..=16`. Scaled power exponents within `1e-9` of an integer are rounded to it.

### 3.9 Functions

| Function | Arity | Argument rule | Result | Value and domain |
| --- | --- | --- | --- | --- |
| `abs(x)` | 1 | any | same as `x` | `abs` |
| `sqrt(x)` | 1 | plain, or every exponent even (else `FractionalDimension`) | exponents halved | `x < 0` is `NegativeSquareRoot`; `sqrt(-0)` is -0 |
| `sin(x)`, `cos(x)`, `tan(x)` | 1 | angle, or plain promoted to an angle (degrees); otherwise `ArgumentDimension { expected: ANGLE }` | plain | radians in, ratio out |
| `asin(x)`, `acos(x)` | 1 | plain; otherwise `ArgumentDimension { expected: NONE }` | angle | `abs(x) > 1 + 1e-12` is `OutOfDomain`; values within the tolerance are clamped to -1 or 1 |
| `atan(x)` | 1 | plain; otherwise `ArgumentDimension { expected: NONE }` | angle | |
| `atan2(y, x)` | 2 | unify `y` and `x` (operation `Atan2`) | angle | `f64::atan2(y, x)`; `atan2(0, 0)` is 0 |
| `min(a, b, ...)`, `max(a, b, ...)` | at least 2 | unify all (operation `Min` or `Max`) | the unified dimension | smallest or largest value |
| `floor(x)`, `ceil(x)`, `round(x)` | 1 | any | same as `x` | plain: applied to the value; otherwise applied in display units: `op(x / f) * f` with `f = display_factor`. `round` rounds half away from zero (`f64::round`) |

Arguments are evaluated left to right before the function's own checks. Argument errors use the argument's span; arity, domain and mismatch errors use the call's span. So `round(1.4 in)` is 36 mm when document units are `mm` (rounded in millimeters) and 25.4 mm when they are `in` (rounded in inches), and `round(30.4 deg)` is 30 degrees.

### 3.10 Expected dimension (implicit units by context)

`evaluate(scope, expected)` evaluates the tree, then:

- `expected == None`: returns the result unchanged. Used for parameters, which have no declared dimension.
- `expected == Some(D)` and the result has dimension D: returned unchanged.
- `expected == Some(D)`, D not plain, and the result is plain: promoted to D (section 3.7). So a bare `10` in a length field is 10 document units, and in an angle field 10 degrees.
- Otherwise: `ExpectedDimension { expected: D, found }` with the root node's span (which excludes surrounding whitespace).

Feature specs state each field's expected dimension; a count field uses `Some(Dimension::NONE)` and rejects `2 mm`. Note that `pi / 4` in an angle field is a plain 0.785398 promoted to degrees, not 45 degrees; write `pi / 4 * rad` or `45`.

### 3.11 Numeric rules

- Every intermediate result (literal, name, operator, function) must be finite; otherwise `NotFinite` with that node's span. Values from parameters and from the context are checked the same way.
- Evaluation uses plain `f64` arithmetic in the order of the tree, so results are deterministic on every target.
- Acceptance tests compare values with a relative or absolute tolerance of `1e-12` unless they state an exact value.

### 3.12 Evaluation order and the first error

Evaluation is post-order, left to right: a `Binary` node evaluates its left operand, then its right, then itself; a call evaluates its arguments in order, then itself. The first error met stops evaluation and is returned. Parse errors come before any evaluation error.

Name resolution:

- `Parameter(name)`: `scope.parameters.get(name)`. Missing gives `UnknownName { name, suggestion }`, with the suggestion of section 3.18 among the names in `scope.parameters`; `Some(Err(_))` gives `DependsOnFailed { name }`; `Some(Ok(q))` gives `q`. Both errors use the name's span.
- `Value(name)`: `scope.context.resolve(name)`. `None` gives `UnknownValue` with the name's span.

### 3.13 Parameter evaluation

`Document::evaluate_parameters(context)` returns a `ParameterValues` with one entry per parameter, in document order, keyed by name:

1. Parse every parameter's expression. A parameter that fails to parse gets that error.
2. Build the dependency graph over the parameters that parsed: an edge from parameter P to parameter Q when Q's name is in `P.expr.references().parameters`. Names that are not parameters add no edge.
3. Every parameter in a strongly connected component with two or more members, or with an edge to itself, gets `Cycle { names }` with the component's names in document order, and the span of its whole source text (`0..source.len()`).
4. Evaluate the remaining parameters in dependency order (each after every parameter it references), breaking ties by document order, each with `expected = None` and a scope made of the document's units, `context`, and every result known so far (the parse errors from step 1, the cycle errors from step 3 and the values and errors already produced in this step). A reference to a failed parameter gives `DependsOnFailed`; a reference to a name that is not a parameter gives `UnknownName`, whose suggestion (section 3.18) is chosen among the names of every parameter of the document except the one being evaluated, so it does not depend on the evaluation order; the first error in evaluation order wins (section 3.12).

Results do not depend on the order of parameters in the document. A parameter whose expression has no units is plain and is promoted wherever it is used: `wall = "2"` is 2 mm in a length field under `mm` and 2 in under `in`.

When the document has two parameters with the same name (impossible for structurally valid documents), the first one wins for lookups.

### 3.14 References

`references()` walks the tree in source order and collects `Parameter` names into `parameters` and `Value` names into `values`, keeping the first occurrence of each. Units, `pi` and function names are not references. Recompute uses this to know which parameters an expression depends on, and validation uses it to report unknown names.

### 3.15 Renaming a parameter

`rename_parameter(from, to)`:

1. `to` must be a valid parameter name (model.md section 3.3), otherwise `RenameError::InvalidName`.
2. The expression must parse, otherwise `RenameError::Parse`.
3. Every `Parameter` node whose name equals `from` has its source bytes replaced by `to`. Nothing else changes: whitespace, number spelling, dotted names that contain `from` as a segment, and identifiers that merely contain `from` (`wall_2`) are untouched. If nothing matches, the result equals the input.

`Document::rename_parameter_edits` (model.md section 3.13) uses this to rename a parameter everywhere in one transaction.

### 3.16 Display text

`Dimension::unit_label(units)` is empty for `NONE`; otherwise the document unit symbol with the length exponent, then `deg` with the angle exponent, each omitted when zero, written `^n` only when n is not 1, joined by `*`. Examples: `LENGTH` in inches is `in`; `AREA` in millimeters is `mm^2`; (1, 1) in millimeters is `mm*deg`; (-1, 0) in centimeters is `cm^-1`.

`Quantity::display(units)`:

1. `v = value / dimension.display_factor(units)`.
2. If `abs(v) < 1e15`, round to 6 decimal places: `v = (v * 1e6).round() / 1e6`.
3. If `v == 0` (including -0), use `0`.
4. Write `v` with Rust's `Display` for `f64` (no exponent: `0.3`, `25.4`, `100000000000000000000`).
5. Plain: the number alone. `LENGTH` or `ANGLE`: the number, a space, and the unit label (`2.4 mm`, `30 deg`, `1 in`). Any other dimension: the number, ` * `, and the unit label (`4 * mm^2`, `5 * cm^-1`).

The text always parses back to the same dimension and, within the rounding of step 2, the same value.

### 3.17 Bare numbers

An expression is a *bare number* when it parses to a `Number` node without a unit, or to one or more `Negate` nodes around one, with no parentheses anywhere: `2`, ` 2 `, `-2`, `- 2`, `.5`, `1e3`. Not bare: `2 mm`, `(2)`, `wall`, `pi`, `2 * 3`, and any text that does not parse. A bare number is the one kind of expression whose value the document units decide by themselves when it stands in a length field (section 3.10), so it is what changes when the units change.

`is_bare_number()` tells whether the expression is one. `with_unit(unit)` returns, for a bare number, the source text with a space and `unit.symbol()` inserted right after the number literal (the end of the `Number` node's span), everything else kept byte for byte: `40` gives `40 mm`, ` 20 ` gives ` 20 mm `, `-2` with `Unit::Inch` gives `-2 in`; for anything else it returns `None`. The result evaluates to the value the bare number has when `unit` is the document unit: `2` under `in` and `2 in` under any units are both 50.8 mm in a length field. model.md section 3.12 uses both to keep sizes when the units change, and features use `is_bare_number` for their sentence that a number without a unit means the document units (features.md section 4.3).

### 3.18 Suggestions for unknown names

`edit_distance(a, b)` is the Levenshtein distance between `a` and `b`: the least number of single-character insertions, deletions and substitutions that turn one into the other, counted in Unicode scalar values and case-sensitive (`hieght` to `height` is 2, `Wall` to `wall` is 1).

An `UnknownName { name }` gets a suggestion when exactly one candidate name other than `name` has `edit_distance(name, candidate) <= SUGGESTION_DISTANCE`; that candidate is the suggestion. With none or several such candidates there is no suggestion, so a suggestion is never a choice between lookalikes. The candidates are the parameter names in the scope (section 3.12), or, inside `Document::evaluate_parameters`, every parameter of the document except the one being evaluated (section 3.13). Dotted names, units and functions get no suggestion.

The suggestion changes the message (section 4) and gives a fix: `ExprError::corrected(source)` is `source.rename_parameter(name, suggestion)` (section 3.15), which replaces every reference to the unknown name and nothing else. Features offer it as a one-click fix of their `expression` error (features.md section 4.1), which hosts label `Replace {name} with {suggestion}` (newcomer error corpus NC10: `Distance: 'hieght' is not a parameter. Did you mean 'height'?` with [Replace hieght with height]).

## 4. Errors

Every `ExprError` has a `kind` and a `span`. The message is the kind's `Display`. `{describe(x)}` is `Dimension::describe`; `{list}` joins names with `, ` and puts ` and ` before the last (`a and b`, `a, b and c`).

| Kind | Span | Message |
| --- | --- | --- |
| `Empty` | whole source, `0..len` | `Enter a value, such as 2 mm.` |
| `TooLong { length: usize }` | `0..len` | `This expression is {length} characters long; the limit is 1000. Move parts of it into parameters.` |
| `TooDeep` | `0..len` | `This expression is nested more than 64 levels deep. Move parts of it into parameters.` |
| `UnexpectedCharacter { character: char }` | the character | `The character '{character}' cannot be used in an expression.` |
| `UnexpectedToken { found: String }` | the token | `Unexpected '{found}'. Check for a missing operator such as * or +.` |
| `UnexpectedEnd` | `len..len` | `The expression ends too early. Add the missing value.` |
| `UnclosedParenthesis` | the `(` | `This ( has no matching ). Add the closing parenthesis.` |
| `UnmatchedParenthesis` | the `)` | `This ) has no matching (. Remove it or add an opening parenthesis.` |
| `PowerOperator` | the `**` | `Use ^ for powers, as in 2^3.` |
| `DecimalComma` | the `,` | `Use . as the decimal separator, as in 2.5.` |
| `UnknownUnit { number: String, unit: String }` | the unit token | `Unknown unit '{unit}' after {number}. Use one of um, mm, cm, m, in, ft, deg or rad, or write {number} * {unit} to multiply.` |
| `UnknownFunction { name: String }` | the name | `Unknown function '{name}'. Available functions: abs, sqrt, sin, cos, tan, asin, acos, atan, atan2, min, max, floor, ceil, round.` |
| `MissingCall { function: Function }` | the name | `{function} is a function. Write {function}(...) with its argument in parentheses.` |
| `ArgumentCount { function: Function, arity: Arity, found: usize }` | the call | `{function}() takes {arity}, but got {found}.` |
| `UnknownName { name: String, suggestion: None }` | the name | `Unknown name '{name}'. Check the spelling, or add a parameter named {name}.` |
| `UnknownName { name, suggestion: Some(suggestion) }` | the name | `'{name}' is not a parameter. Did you mean '{suggestion}'?` |
| `UnknownValue { name: String }` | the name | `Unknown value '{name}'. Check the spelling; printer values look like printer.fit.slip.` |
| `DependsOnFailed { name: String }` | the name | `This uses {name}, which has an error. Fix {name} first.` |
| `Cycle { names: Vec<String> }`, one name | `0..len` | `{name} depends on itself. Change its expression so it does not use {name}.` |
| `Cycle { names }`, two or more | `0..len` | `{list} depend on each other in a loop. Change one of them so it does not use the others.` |
| `DimensionMismatch { operation: Add, left, right }` | the `+` node | `Cannot add {describe(left)} and {describe(right)}. Both sides need the same kind of value.` |
| `DimensionMismatch { operation: Subtract, left, right }` | the `-` node | `Cannot subtract {describe(right)} from {describe(left)}. Both sides need the same kind of value.` |
| `DimensionMismatch { operation: Min or Max or Atan2, left, right }` | the call | `{function}() needs values of the same kind, but got {describe(left)} and {describe(right)}.` |
| `ArgumentDimension { function: Function, expected: Dimension, found: Dimension }` | the argument | `{function}() takes {describe(expected)}, but got {describe(found)}.` |
| `ExponentDimension { found: Dimension }` | the exponent | `The exponent must be a plain number, but got {describe(found)}.` |
| `FractionalDimension { found: Dimension }` | the `^` node or the call | `Cannot take a fractional power of {describe(found)}: the result would have no unit.` |
| `DimensionOverflow` | the node | `The units in this expression grow beyond a power of 16. Simplify the expression.` |
| `ExpectedDimension { expected: Dimension, found: Dimension }` | the root node | `Expected {describe(expected)}, but this expression gives {describe(found)}.` |
| `DivisionByZero` | the `/` node | `Division by zero. Check the value after the /.` |
| `NegativeSquareRoot` | the call | `sqrt() needs a value of 0 or more.` |
| `OutOfDomain { function: Function }` | the call | `{function}() needs a value from -1 to 1.` |
| `NegativeBase` | the `^` node | `A negative number cannot be raised to a fractional power.` |
| `NotFinite` | the node | `The result is too large to use.` |

`{function}` in `DimensionMismatch` is `min`, `max` or `atan2` according to the operation. `{character}` is written with `char::escape_debug`. In `UnknownUnit`, `number` is the number literal exactly as written (`2`, `1.5e3`) and `unit` is the name token's text.

`RenameError` displays its inner error's message. `NameError` is in model.md section 4.3.

## 5. Performance budgets

Release build on the reference desktop (2022 mid-range laptop); web budgets are 3 times these. Checked by CI benchmarks.

| Operation | Desktop budget |
| --- | --- |
| `parse` of a 100-character expression | 5 microseconds |
| `evaluate` of a 100-character expression with 5 parameter references | 10 microseconds |
| `evaluate_parameters` for 1,000 parameters in one chain (each uses the previous) | 5 ms |
| `evaluate_parameters` for 1,000 independent parameters | 5 ms |
| `rename_parameter` on a 100-character expression | 10 microseconds |

## 6. Acceptance tests

**Scope S(units)**: parameters from `evaluate_parameters` on a document with these parameters, in order: `wall` = `2 mm`, `ratio` = `0.5`, `angle` = `30 deg`, `bad` = `1 / 0`; context: a `BTreeMap` with `printer.fit.slip` mapped to `Quantity::length(0.2)` and `printer.nozzle` to `Quantity::length(0.4)`; `units` as given (default `Mm`).

`eval(src)` means `Expr::new(src).evaluate(&S(Mm), None)`; `eval_in(units, src, expected)` uses `S(units)` and `expected`. "length 2" means `Ok(Quantity::length(2.0))`, "plain 2" means `Ok(Quantity::plain(2.0))`, "area 4" means `Ok(Quantity::new(4.0, Dimension::AREA))`, "angle x" means `Ok(Quantity::angle(x))`. Errors are written as kind plus span, such as `UnexpectedEnd @ 3..3`; `UnknownName { "wal" }` means `UnknownName { name: "wal", suggestion: None }`. Values compare within `1e-12` relative or absolute unless stated.

### Literals and syntax

1. `eval("2")` is plain 2; `eval("2 mm")` and `eval("2mm")` are length 2; `eval(".5 mm")` is length 0.5; `eval("5.")` is plain 5; `eval("2.e3")` is plain 2000.
2. `eval("1e3")` is plain 1000; `eval("1E-3 m")` is length 1; `eval("2.5e+1")` is plain 25.
3. `eval("2 + 3 * 4")` is 14; `eval("(2 + 3) * 4")` is 20; `eval("10 - 4 - 3")` is 3; `eval("64 / 4 / 2")` is 8.
4. `eval("2 ^ 3 ^ 2")` is 512; `eval("-2 ^ 2")` is -4; `eval("(-2) ^ 2")` is 4; `eval("2 ^ -1")` is 0.5; `eval("2 ^ -2 ^ 2")` is 0.0625.
5. `eval("--2")` is plain 2; `eval("2 * -3")` is plain -6; `eval("-(2 mm)")` is length -2.
6. `eval(" \t2\n*\r3 ")` is plain 6.
7. `eval("2 mm ^ 2")` is area 4; `eval("2 * mm ^ 2")` is area 2; `eval("mm")` is length 1.
8. `eval("sqrt(16 mm^2)")` is length 16; `eval("sqrt(16 * mm^2)")` is length 4.
9. `eval("wall / 2 mm")` is plain 1.
10. `eval("30°")`, `eval("30 °")` and `eval("30 deg")` are angle `PI / 6`.
11. `Expr::new("2 * wall + sin(30 deg)").parse()` has root `Binary { Add }` at `0..22`; its left is `Binary { Multiply }` at `0..8` with `Number { 2, None }` at `0..1` and `Parameter("wall")` at `4..8`; its right is `Call { Sin }` at `11..22` with one argument `Number { 30, Some(Degree) }` at `15..21`.
12. `Expr::new("(1 + 2) * 3").parse()` has root `Binary { Multiply }` at `0..11`; its left is `Binary { Add }` at `0..7` (parentheses included) whose left is `Number { 1, None }` at `1..2`.

### Units and names

13. `eval` of `1000 um`, `1 mm`, `1 cm`, `1 m`, `1 in`, `1 ft` gives lengths 1, 1, 10, 1000, 25.4, 304.8; `180 deg` gives angle `PI`; `1 rad` gives angle 1.
14. `eval("1 in + 1 mm")` is length 26.4.
15. `eval("pi")` is plain `PI`; `eval("pi / 4 * rad")` is angle `PI / 4`.
16. `eval("printer.fit.slip")` is length 0.2; `eval("ratio * 4")` is plain 2.
17. `Unit::from_symbol("°")` is `Some(Degree)`; `Unit::from_symbol("MM")` is `None`; `Function::from_name("atan2")` is `Some(Atan2)`; `Function::Min.arity()` is `AtLeast(2)`; `Arity::AtLeast(2).to_string()` is `at least 2 arguments`; `Arity::Exactly(1).to_string()` is `1 argument`.

### Parse errors

18. `eval("")` is `Empty @ 0..0`; `eval("   ")` is `Empty @ 0..3`.
19. `" ".repeat(999) + "1"` (1000 characters) evaluates to plain 1; `" ".repeat(1000) + "1"` is `TooLong { length: 1001 } @ 0..1001`.
20. `eval("2 $ 3")` is `UnexpectedCharacter { '$' } @ 2..3`; `eval("2 + + $")` is `UnexpectedCharacter { '$' } @ 6..7` (lexing first); `eval("wäll")` is `UnexpectedCharacter { 'ä' } @ 1..3`.
21. `eval("2 +")` is `UnexpectedEnd @ 3..3`; `eval("sqrt(")` is `UnexpectedEnd @ 5..5`.
22. `eval("(2 + 3")` is `UnclosedParenthesis @ 0..1`; `eval("((1) + 2")` is `UnclosedParenthesis @ 0..1`.
23. `eval("2 + 3)")` is `UnmatchedParenthesis @ 5..6`.
24. `eval("2 ** 3")` is `PowerOperator @ 2..4`.
25. `eval("2,5")` is `DecimalComma @ 1..2`; `eval("(2,5)")` is `DecimalComma @ 2..3`; `eval("max(2,5)")` is plain 5.
26. `eval("2 3")` is `UnexpectedToken { "3" } @ 2..3`; `eval("2 (3)")` is `UnexpectedToken { "(" } @ 2..3`; `eval("()")` is `UnexpectedToken { ")" } @ 1..2`.
27. `eval("2 inch")` is `UnknownUnit { number: "2", unit: "inch" } @ 2..6`, with message `Unknown unit 'inch' after 2. Use one of um, mm, cm, m, in, ft, deg or rad, or write 2 * inch to multiply.`
28. `eval("2 wall")` is `UnknownUnit { number: "2", unit: "wall" } @ 2..6`; `eval("2em")` is `UnknownUnit { number: "2", unit: "em" } @ 1..3`.
29. `eval("foo(2)")` is `UnknownFunction { "foo" } @ 0..3`; `eval("wall(2)")` is `UnknownFunction { "wall" } @ 0..4`.
30. `eval("sqrt + 1")` is `MissingCall { Sqrt } @ 0..4`, with message `sqrt is a function. Write sqrt(...) with its argument in parentheses.`
31. `eval("min(1)")` is `ArgumentCount { Min, AtLeast(2), 1 } @ 0..6` with message `min() takes at least 2 arguments, but got 1.`; `eval("sin(1, 2)")` is `ArgumentCount { Sin, Exactly(1), 2 } @ 0..9`; `eval("sqrt()")` is `ArgumentCount { Sqrt, Exactly(1), 0 } @ 0..6`.
32. `eval("max(1, 2,)")` is `UnexpectedToken { ")" } @ 9..10`.
33. Depth: `"(" * 64 + "1" + ")" * 64` is plain 1; `"(" * 65 + "1" + ")" * 65` is `TooDeep`; `"-" * 63 + "1"` is plain -1; `"-" * 64 + "1"` is `TooDeep`; `"1" + "+1" * 63` is plain 64; `"1" + "+1" * 64` is `TooDeep`; `"(" * 1000` is `TooDeep` (and does not panic).
34. Robustness: every string of length 1 to 4 over the characters `1 m ( ) + - * ^ , . ° e` and space parses (and, if it parses, evaluates in `S(Mm)`) without panicking.

### Evaluation errors

35. `eval("wal")` is `UnknownName { "wal" } @ 0..3` with message `Unknown name 'wal'. Check the spelling, or add a parameter named wal.` (both `wall` and `bad` are within edit distance 2 of `wal`, so there is no suggestion, section 3.18).
36. `eval("printer.fit.slipp")` is `UnknownValue { "printer.fit.slipp" } @ 0..17`.
37. `eval("bad * 2")` is `DependsOnFailed { "bad" } @ 0..3`.
38. `eval("1/0")` is `DivisionByZero @ 0..3`; `eval("1 mm / (2 mm - 2 mm)")` is `DivisionByZero @ 0..20`; `eval("0/0")` is `DivisionByZero @ 0..3`.
39. `eval("10^400")` is `NotFinite @ 0..6`; `eval("1e400")` is `NotFinite @ 0..5`; `eval("1e308 * 10")` is `NotFinite @ 0..10`.
40. `eval("(-8)^(1/3)")` is `NegativeBase @ 0..10`; `eval("(-8)^3")` is plain -512.
41. `eval("sqrt(-4)")` is `NegativeSquareRoot @ 0..8`; `eval("sqrt(-4 mm^2)")` is `NegativeSquareRoot @ 0..13` (it is the square root of -(4 mm)^2).
42. `eval("asin(2)")` is `OutOfDomain { Asin } @ 0..7`; `eval("acos(-1.5)")` is `OutOfDomain { Acos } @ 0..10`.
43. `eval("asin(1 mm)")` is `ArgumentDimension { Asin, expected: NONE, found: LENGTH } @ 5..9`.
44. `eval("sin(2 mm)")` is `ArgumentDimension { Sin, expected: ANGLE, found: LENGTH } @ 4..8` with message `sin() takes an angle, but got a length.`
45. `eval("2 mm + 30 deg")` is `DimensionMismatch { Add, LENGTH, ANGLE } @ 0..13` with message `Cannot add a length and an angle. Both sides need the same kind of value.`
46. `eval("2 mm - 1 deg")` gives the message `Cannot subtract an angle from a length. Both sides need the same kind of value.`
47. `eval("max(1 mm, 2, 1 deg)")` is `DimensionMismatch { Max, LENGTH, ANGLE } @ 0..19`; `eval("atan2(1 mm, 1 deg)")` is `DimensionMismatch { Atan2, LENGTH, ANGLE } @ 0..18`.
48. `eval("wall ^ wall")` is `ExponentDimension { LENGTH } @ 7..11`.
49. `eval("sqrt(2 mm)")` is `FractionalDimension { LENGTH } @ 0..10`; `eval("(2 mm) ^ 0.5")` is `FractionalDimension { LENGTH } @ 0..12`.
50. `eval("mm ^ 17")` is `DimensionOverflow @ 0..7`; `eval("mm ^ 16")` is `Ok` with dimension `{ length: 16, angle: 0 }`.
51. `eval("(2 mm * 2 mm) ^ 0.5")` is length 2.
52. When two errors are present, the leftmost in evaluation order wins: `eval("wal + 1/0")` is `UnknownName`; `eval("1/0 + wal")` is `DivisionByZero`.

### Plain numbers and promotion

53. `eval_in(Mm, "10 + wall", None)` is length 12; `eval_in(In, "10 + wall", None)` is length 256.
54. `eval("2 * wall")` is length 4; `eval("wall * wall")` is area 4; `eval("wall / 4")` is length 0.5.
55. `eval("wall + ratio")` is length 2.5.
56. `eval("sin(30)")`, `eval("sin(30 deg)")` and `eval("cos(pi / 3 * rad)")` are plain 0.5; `eval("tan(45)")` is plain 1; `eval("sin(angle)")` is plain 0.5.
57. `eval("atan2(1, 1)")` is angle `PI / 4`; `eval_in(Mm, "atan2(3 mm, 4)", None)` is angle `0.6435011087932844`; `eval_in(Cm, "atan2(3 mm, 4)", None)` is angle `0.07485984771076686`; `eval("atan2(0, 0)")` is angle 0.
58. `eval("asin(0.5)")` is angle `PI / 6`; `eval("acos(1)")` is angle 0; `eval("atan(1)")` is angle `PI / 4`; `eval("asin(1 + 1e-13)")` is angle `PI / 2`.
59. `eval("floor(2.7)")` is plain 2; `eval("ceil(2.1)")` is 3; `eval("round(2.5)")` is 3; `eval("round(-2.5)")` is -3; `eval("floor(-0.5)")` is -1.
60. `eval_in(Mm, "round(1.4 in)", None)` is length 36; `eval_in(In, "round(1.4 in)", None)` is length 25.4; `eval("round(30.4 deg)")` is angle `PI / 6`.
61. `eval("floor(wall * 1.5)")` is length 3; `eval_in(Cm, "floor(2.7 mm)", None)` is length 0.
62. `eval("min(3, wall)")` is length 2; `eval("max(1 mm, 2 mm, 0.5 mm)")` is length 2; `eval("min(3, 4, 1)")` is plain 1; `eval("abs(-3 mm)")` is length 3.

### Expected dimension

63. `eval_in(u, "10", Some(LENGTH))` is length 10, 100, 10000 and 254 for `u` = `Mm`, `Cm`, `M`, `In`.
64. `eval_in(Mm, "45", Some(ANGLE))` is angle `PI / 4`; `eval_in(Mm, "pi / 4", Some(ANGLE))` is angle `0.013707783890401887`; `eval_in(Mm, "pi / 4 * rad", Some(ANGLE))` is angle `PI / 4`.
65. `eval_in(Mm, "2 mm", Some(ANGLE))` is `ExpectedDimension { ANGLE, LENGTH } @ 0..4` with message `Expected an angle, but this expression gives a length.`; `eval_in(Mm, " 2 mm ", Some(ANGLE))` has span `1..5`.
66. `eval_in(Mm, "2 mm", Some(NONE))` is `ExpectedDimension { NONE, LENGTH }`; `eval_in(Mm, "wall / 1 mm", Some(NONE))` is plain 2; `eval_in(Mm, "4", Some(NONE))` is plain 4.
67. `eval_in(Cm, "10", Some(AREA))` is area 1000; `eval_in(Mm, "2 mm", Some(LENGTH))` is length 2; `eval_in(Mm, "ratio", Some(LENGTH))` is length 0.5.

### Parameters

68. Parameters in document order `c` = `b + a`, `b` = `a * 2`, `a` = `2 mm` evaluate to `c` length 6, `b` length 4, `a` length 2, and `iter()` yields names `c`, `b`, `a` in that order.
69. Parameters `a` = `b`, `b` = `a`, `d` = `a + 1`: `a` and `b` are `Cycle { names: ["a", "b"] } @ 0..1`; `d` is `DependsOnFailed { "a" } @ 0..1`. The message for `a` is `a and b depend on each other in a loop. Change one of them so it does not use the others.`
70. Parameter `a` = `a + 1` is `Cycle { names: ["a"] } @ 0..5` with message `a depends on itself. Change its expression so it does not use a.`
71. Parameters `a` = `b + x`, `b` = `a`: both are `Cycle { names: ["a", "b"] }` (cycles win over the unknown name `x`).
72. Parameters `a` = `wal * 2`, `b` = `a`: `a` is `UnknownName { "wal" } @ 0..3`; `b` is `DependsOnFailed { "a" } @ 0..1`.
73. Parameters `a` = `1 +`, `b` = `a`: `a` is `UnexpectedEnd @ 3..3`; `b` is `DependsOnFailed { "a" }`.
74. Parameter `a` = `printer.fit.slip * 2` is length 0.4 with the S context and `UnknownValue { "printer.fit.slip" } @ 0..16` with `EmptyContext`.
75. In a document with units `In`, parameter `a` = `10 + 2 mm` is length 256 and parameter `b` = `2` is plain 2.
76. A chain of 1,000 parameters `p0` = `1 mm` and `p{i}` = `p{i-1} + 1 mm` evaluates `p999` to length 1000.
77. `ParameterValues::from_iter([("a", Ok(plain 1)), ("a", Ok(plain 2))])` has `len() == 1` and `get("a")` is plain 1.

### References and rename

78. `Expr::new("2 * wall + printer.fit.slip + wall + mm + sqrt(depth) + pi").references()` is `Ok(References { parameters: ["wall", "depth"], values: ["printer.fit.slip"] })`.
79. `Expr::new("1 +").references()` is `Err` with kind `UnexpectedEnd`.
80. `Expr::new("wall*2 + wall_2 + printer.wall + wall").rename_parameter("wall", "thickness")` is `Ok("thickness*2 + wall_2 + printer.wall + thickness")`.
81. `Expr::new("  wall  +1").rename_parameter("wall", "t")` is `Ok("  t  +1")`; `Expr::new("2 mm").rename_parameter("mm", "x")` is `Ok("2 mm")`; `Expr::new("depth").rename_parameter("wall", "t")` is `Ok("depth")`.
82. `rename_parameter("wall", "2x")` is `Err(InvalidName(InvalidStart { .. }))`; `rename_parameter("wall", "mm")` is `Err(InvalidName(Reserved { .. }))`; `Expr::new("wall +").rename_parameter("wall", "t")` is `Err(Parse(..))` with kind `UnexpectedEnd`.

### Dimensions, quantities and display

83. `describe()` gives `a plain number`, `a length`, `an area`, `a volume`, `an angle` for `NONE`, `LENGTH`, `AREA`, `VOLUME`, `ANGLE`, and `a quantity in mm^-1*rad` for `{ length: -1, angle: 1 }`.
84. `unit_label`: `LENGTH` with `In` is `in`; `AREA` with `Mm` is `mm^2`; `{ length: 1, angle: 1 }` with `Mm` is `mm*deg`; `{ length: -1, angle: 0 }` with `Cm` is `cm^-1`; `NONE` is empty. `AREA.display_factor(In)` is `645.16` (within `1e-12` relative).
85. `Quantity::length(25.4).display(In)` is `1 in`; `.display(Mm)` is `25.4 mm`; `Quantity::length(2.4000000000000004).display(Mm)` is `2.4 mm`; `Quantity::angle(PI / 6).display(Mm)` is `30 deg`.
86. `Quantity::new(4.0, AREA).display(Mm)` is `4 * mm^2`; `Quantity::new(0.5, { length: -1, angle: 0 }).display(Cm)` is `5 * cm^-1`.
87. `Quantity::plain(0.30000000000000004).display(Mm)` is `0.3`; `Quantity::plain(-0.0000001).display(Mm)` is `0`; `Quantity::length(1e20).display(Mm)` is `100000000000000000000 mm`.
88. Property (1,000 cases): for a random quantity (value in `-1e6..1e6`, dimension one of `NONE`, `LENGTH`, `AREA`, `ANGLE`, `{ length: -1, angle: 0 }`, `{ length: 1, angle: 1 }`, random document units), `Expr::new(q.display(units)).evaluate(&scope_with(units), None)` has the same dimension and a value within `1e-6 * display_factor` of `q.value`.
89. `serde_json::to_value(Dimension::LENGTH)` is `{"length": 1, "angle": 0}`; `serde_json::to_value(Quantity::length(25.4))` is `{"value": 25.4, "dimension": {"length": 1, "angle": 0}}` with keys in that order.
90. `serde_json::to_string(&Expr::new(" 2 mm"))` is `"\" 2 mm\""`; `serde_json::from_str::<Expr>("2")` fails.
91. `EmptyContext.resolve("printer.fit.slip")` is `None`; the S context's `names()` is `["printer.fit.slip", "printer.nozzle"]`.

### Bare numbers and suggestions

92. In a document with units `Mm` and parameters `height` = `30 mm` and `width` = `80 mm`, `Expr::new("hieght").evaluate(&scope, Some(LENGTH))` is `UnknownName { name: "hieght", suggestion: Some("height") } @ 0..6` with message `'hieght' is not a parameter. Did you mean 'height'?`, and `corrected(&Expr::new("hieght"))` is `Some("height")`. In the same scope, `Expr::new("2 * hieght + hieght")` gives `UnknownName { name: "hieght", suggestion: Some("height") } @ 4..10`, whose `corrected` of that source is `Some("2 * height + height")`; an error without a suggestion (test 35) gives `None`.
93. Exactly one close name: in `S(Mm)`, `eval("rati")` is `UnknownName { name: "rati", suggestion: Some("ratio") } @ 0..4` with message `'rati' is not a parameter. Did you mean 'ratio'?`; `eval("xyz")` has no suggestion; `eval("wal")` has none either (test 35); `eval("printer.fit.slipp")` stays `UnknownValue` (test 36).
94. `edit_distance` is 2 for `hieght` and `height`, 2 for `wal` and `bad`, 3 for the empty text and `abc`, 0 for `wall` and `wall`, 1 for `Wall` and `wall`, 1 for `é` and `e`, and 3 for `kitten` and `sitting`.
95. Parameters `a` = `heigth + 1` and `height` = `30 mm`, in this order and in the reverse order, give `a` the error `UnknownName { name: "heigth", suggestion: Some("height") } @ 0..6`. A document whose only parameter is `height` = `heigth + 1` gives `UnknownName { name: "heigth", suggestion: None }`: a parameter is never suggested inside its own expression.
96. `is_bare_number` is true for `2`, ` 2 `, `-2`, `- 2`, `--2`, `.5` and `1e3`, and false for `2 mm`, `(2)`, `-(2)`, `wall`, `pi`, `2 * 3`, `2,5` and the empty text.
97. `with_unit`: `Expr::new("40").with_unit(Unit::Millimeter)` is `Some("40 mm")`; `" 20 "` gives `Some(" 20 mm ")`; `"-2"` with `Unit::Inch` gives `Some("-2 in")`; `"- 2"` gives `Some("- 2 mm")`; `"1e3"` gives `Some("1e3 mm")`; `"(2)"`, `"2 mm"` and `"wall"` give `None`. `eval_in(In, "2", Some(LENGTH))` and `eval_in(Mm, "2 in", Some(LENGTH))` are both length 50.8.

## 7. Open issues

1. **Plain-number promotion in sums.** Architecture says evaluation is dimension-checked and bare numbers take document units or degrees by context, but not how `10 + wall` behaves. This spec promotes plain numbers in `+`, `-`, `min`, `max`, `atan2`, trig arguments and against the field's expected dimension (as Fusion does), and treats every dimensionless value as plain, including ratios. The stricter alternative (bare numbers only at the top level) is simpler but rejects common input such as `wall + 0.2`.
2. **Unit names as values.** Not in architecture's grammar list. Added so `pi / 4 * rad`, `(a + b) * mm` and `2 * mm^2` can be written, mirroring the Python API's `2*mm`. It is why unit names are reserved as parameter names (`m` cannot be a parameter).
3. **The degree sign `°` and the units `um` and `ft`.** Architecture's examples name `mm`, `deg` and `in`; this spec adds `cm`, `m`, `um`, `ft`, `rad` and `°`.
4. **Angles are a dimension.** Architecture lists angle as a dimension, so `sin(wall / thickness)` reads the ratio as degrees, and `r * theta` is length times angle, which a length field rejects. Arc lengths need `r * theta / rad`.
5. **`pi` is plain.** `pi / 4` in an angle field is 0.785 degrees, not 45 degrees. The alternative (`pi` as an angle of 180 degrees) breaks `pi * d` in length fields. The UI may show a hint when an angle field contains `pi` without `rad`.
6. **Rounding functions on dimensioned values** round in display units (document units, degrees). Restricting them to plain numbers was the alternative.
7. **Limits.** 1000 characters and depth 64 are this spec's choices, sized so the parser and evaluator stay well inside the small WebAssembly stack.
8. **Spans are UTF-8 byte offsets.** The UI works in UTF-16 code units; the protocol spec must say who converts.
9. **Unknown functions and units are parse errors; unknown names are evaluation errors.** So `Document::validate` reports `wall(2)` as an invalid expression but `wal` as an unknown parameter.
10. **Suggestions for unknown names.** Resolved for parameter names by section 3.18, which keeps messages exact and testable: a suggestion is made only when exactly one parameter is within edit distance 2, so `wal` gets none in a scope with both `wall` and `bad`. A relative limit (distance at most a third of the name's length, as some compilers use) would suggest `wall` there; the fixed limit follows the newcomer error corpus (NC10) and features' `unknown_params` rule. Levenshtein distance counts a swap of two letters as 2, which the limit still allows (`hieght`).
11. **Number text for the Python API.** The features spec says Python records an `int` or `float` as "the canonical number text". The function for that is `canonical::format_number` (file-format.md), applied to `serde_json::Number::from_f64(value)`.
