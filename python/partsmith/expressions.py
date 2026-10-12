"""Expressions, units, the printer namespace and value conversion (python-api.md section 3.2).

An `Expr` is expression text as the document stores it. Python operators build new expressions
whose text follows the precedence rules of expressions.md, with the fewest parentheses that keep
the meaning. Values passed to expression params render to text and stay parametric; values
passed to raw geometry params (sketch coordinates, radii, angles) are converted to numbers in
internal units (millimeters, radians).
"""

from __future__ import annotations

import math
import re
from typing import Protocol

# Precedence levels of expressions.md section 3.4, from loosest to tightest.
SUM = 1
PRODUCT = 2
UNARY = 3
POWER = 4
ATOM = 5

OPERATOR_PRECEDENCE = {"+": SUM, "-": SUM, "*": PRODUCT, "/": PRODUCT, "^": POWER}
"""The level of each binary operator."""

MAX_EXACT_INTEGER = 2.0**53
"""Integral floats below this magnitude render without a fraction."""

UNIT_SYMBOLS = {"mm": "mm", "cm": "cm", "meter": "m", "inch": "in", "deg": "deg", "rad": "rad"}
"""The expression symbol of each Python unit object."""

LENGTH_FACTORS = {"mm": 1.0, "cm": 10.0, "m": 1000.0, "in": 25.4}
"""Millimeters per length unit, also the factor of each document unit."""

DEGREE = math.pi / 180.0
"""Radians per degree, the factor the evaluator uses."""

ANGLE_FACTORS = {"deg": DEGREE, "rad": 1.0}
"""Radians per angle unit."""

LENGTH = (1, 0)
"""The (length, angle) exponents of a length."""

ANGLE = (0, 1)
"""The (length, angle) exponents of an angle."""

PLAIN = (0, 0)
"""The (length, angle) exponents of a plain number."""

_TOKEN = re.compile(
    r"""\s*(?:
        (?P<number>(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?)
      | (?P<name>[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)
      | (?P<symbol>[-+*/^(),°])
    )""",
    re.VERBOSE,
)
"""One token of expression text (expressions.md section 3.2), after optional whitespace."""

_OPERAND_ENDS = frozenset({"number", "name", ")", "°"})
"""Tokens after which a `-` or `+` is binary."""


class Evaluator(Protocol):
    """What raw value conversion needs from a document."""

    @property
    def units(self) -> str:
        """The document units."""
        ...

    def _evaluate(self, source: str) -> tuple[float, tuple[int, int]]:
        """The value in internal units and the (length, angle) exponents of an expression."""
        ...


class Expr:
    """An expression; `str()` is its source text as stored in the document."""

    __slots__ = ("_precedence", "_quantity", "source")

    def __init__(self, source: str) -> None:
        if not isinstance(source, str):
            raise TypeError(f"An expression is text, not {type(source).__name__}.")
        self.source = source
        self._precedence = source_precedence(source)
        self._quantity: tuple[int | float, str] | None = None

    @classmethod
    def _built(
        cls, source: str, precedence: int, quantity: tuple[int | float, str] | None = None
    ) -> Expr:
        """An expression built by Python with a known precedence, and the number and unit symbol
        of a quantity literal (`number * unit`)."""
        built = Expr.__new__(Expr)
        built.source = source
        built._precedence = precedence
        built._quantity = quantity
        return built

    def __str__(self) -> str:
        return self.source

    def __repr__(self) -> str:
        return f"expr({self.source!r})"

    def __add__(self, other: object) -> Expr:
        return _binary(self, "+", other)

    def __radd__(self, other: object) -> Expr:
        return _binary(other, "+", self)

    def __sub__(self, other: object) -> Expr:
        return _binary(self, "-", other)

    def __rsub__(self, other: object) -> Expr:
        return _binary(other, "-", self)

    def __mul__(self, other: object) -> Expr:
        return self._quantity_with(other) or _binary(self, "*", other)

    def __rmul__(self, other: object) -> Expr:
        return self._quantity_with(other) or _binary(other, "*", self)

    def __truediv__(self, other: object) -> Expr:
        return _binary(self, "/", other)

    def __rtruediv__(self, other: object) -> Expr:
        return _binary(other, "/", self)

    def __pow__(self, other: object) -> Expr:
        return _binary(self, "^", other)

    def __rpow__(self, other: object) -> Expr:
        return _binary(other, "^", self)

    def __neg__(self) -> Expr:
        return Expr._built("-" + _operand_text(self.source, self._precedence, UNARY), UNARY)

    def __pos__(self) -> Expr:
        return Expr._built(self.source, self._precedence, self._quantity)

    def _quantity_with(self, number: object) -> Expr | None:
        """The quantity literal `<number> <unit>` when this is a unit object and `number` a plain
        number, else None."""
        if not isinstance(self, _Unit) or not _is_number(number):
            return None
        text = render_number(number, "A quantity")
        precedence = UNARY if text.startswith("-") else ATOM
        return Expr._built(f"{text} {self.symbol}", precedence, (number, self.symbol))


class _Unit(Expr):
    """A unit object: one of its unit, which turns a number into a quantity literal."""

    __slots__ = ("symbol",)

    def __init__(self, symbol: str) -> None:
        super().__init__(f"1 {symbol}")
        self.symbol = symbol
        self._quantity = (1, symbol)


mm = _Unit("mm")
cm = _Unit("cm")
meter = _Unit("m")
inch = _Unit("in")
deg = _Unit("deg")
rad = _Unit("rad")


class _PrinterName(Expr):
    """A dotted `printer.*` name; attribute access extends it."""

    __slots__ = ()

    def __getattr__(self, name: str) -> _PrinterName:
        if name.startswith("_"):
            raise AttributeError(name)
        return _PrinterName(f"{self.source}.{name}")


class PrinterNamespace:
    """`printer.fit.slip` is the expression `printer.fit.slip`; `printer` alone is not a value."""

    __slots__ = ()

    def __getattr__(self, name: str) -> _PrinterName:
        if name.startswith("_"):
            raise AttributeError(name)
        return _PrinterName(f"printer.{name}")

    def __repr__(self) -> str:
        return "printer"


printer = PrinterNamespace()


def expr(source: str) -> Expr:
    """An expression from source text, stored verbatim."""
    return Expr(source)


def render_value(value: object, name: str) -> str:
    """The expression text of a value (python-api.md section 3.2).

    An int renders as itself, a float as the integer when it is integral and below 2**53 in
    magnitude, else as its repr; an Expr renders as its source and a str is used verbatim.
    `name` names the argument in error messages, for example `box() length`.
    """
    if isinstance(value, Expr):
        return value.source
    if isinstance(value, str):
        return value
    if _is_number(value):
        return render_number(value, name)
    raise TypeError(f"{name} must be a number, an Expr or expression text, not {type(value).__name__}.")


def render_number(value: int | float, name: str) -> str:
    """The text of a plain number: an int as itself, an integral float below 2**53 without a
    fraction, any other finite float as its repr."""
    if isinstance(value, int):
        return str(value)
    if not math.isfinite(value):
        raise ValueError(f"{name} must be a finite number, not {value!r}.")
    if value.is_integer() and abs(value) < MAX_EXACT_INTEGER:
        return str(int(value))
    return repr(value)


def raw_length(value: object, document: Evaluator, name: str) -> float:
    """A raw length in millimeters: a plain number in document units, a quantity literal by its
    unit, any other expression evaluated by the engine (a plain result in document units)."""
    return _raw(value, document, name, LENGTH, LENGTH_FACTORS[document.units])


def raw_angle(value: object, document: Evaluator, name: str) -> float:
    """A raw angle in radians: a plain number in degrees, a quantity literal by its unit, any
    other expression evaluated by the engine (a plain result in degrees)."""
    return _raw(value, document, name, ANGLE, DEGREE)


def raw_number(value: object, name: str) -> float:
    """A finite plain number, for unit-free raw values such as placement axes."""
    if not _is_number(value):
        raise TypeError(f"{name} must be a number, not {type(value).__name__}.")
    number = float(value)
    if not math.isfinite(number):
        raise ValueError(f"{name} must be a finite number, not {value!r}.")
    return number


def is_quantity(value: object) -> bool:
    """True for a quantity literal built as `number * unit` (unit objects included)."""
    return isinstance(value, Expr) and value._quantity is not None


def describe_dimension(dimension: tuple[int, int]) -> str:
    """The name of a dimension in an error message (expressions.md `Dimension::describe`)."""
    names = {PLAIN: "a plain number", LENGTH: "a length", ANGLE: "an angle", (2, 0): "an area", (3, 0): "a volume"}
    return names.get(dimension, "another kind of quantity")


def source_precedence(source: str) -> int:
    """The precedence level of the top-level operation in expression text.

    Text that does not tokenize cleanly gets the loosest level, so it is always parenthesized
    when it becomes an operand.
    """
    position = 0
    depth = 0
    previous: str | None = None
    first: str | None = None
    levels: set[int] = set()
    while position < len(source):
        if not source[position:].strip():
            break
        match = _TOKEN.match(source, position)
        if match is None:
            return SUM
        position = match.end()
        kind = match.lastgroup
        token = match.group(kind) if kind == "symbol" else kind
        if first is None:
            first = token
        if token == "(":
            depth += 1
        elif token == ")":
            depth -= 1
        elif depth == 0 and token in OPERATOR_PRECEDENCE:
            binary = token in "*/^" or previous in _OPERAND_ENDS
            if binary:
                levels.add(OPERATOR_PRECEDENCE[token])
        previous = token
    if first is None or depth != 0:
        return SUM
    if levels & {SUM, PRODUCT}:
        return min(levels)
    if first == "-":
        return UNARY
    return min(levels, default=ATOM)


def _is_number(value: object) -> bool:
    """True for an int or float that is not a bool."""
    return isinstance(value, (int, float)) and not isinstance(value, bool)


def _operand(value: object) -> tuple[str, int]:
    """The text and precedence of an operator's operand."""
    if isinstance(value, Expr):
        return value.source, value._precedence
    if isinstance(value, str):
        return value, source_precedence(value)
    text = render_value(value, "An expression operand")
    return text, UNARY if text.startswith("-") else ATOM


def _operand_text(text: str, precedence: int, level: int, *, strict: bool = False) -> str:
    """The operand text, parenthesized when its precedence is lower than `level` (or equal,
    with `strict`)."""
    if precedence < level or (strict and precedence == level):
        return f"({text})"
    return text


def _binary(left: object, operator: str, right: object) -> Expr:
    """`left <operator> right` with one space around the operator and the parentheses that
    keep the meaning."""
    if not _is_operand(left) or not _is_operand(right):
        return NotImplemented
    level = OPERATOR_PRECEDENCE[operator]
    left_text, left_precedence = _operand(left)
    right_text, right_precedence = _operand(right)
    left_part = _operand_text(left_text, left_precedence, level, strict=operator == "^")
    right_part = _operand_text(right_text, right_precedence, level, strict=operator in "-/")
    return Expr._built(f"{left_part} {operator} {right_part}", level)


def _is_operand(value: object) -> bool:
    """True for values an operator accepts: numbers, expressions and expression text."""
    return isinstance(value, (Expr, str)) or _is_number(value)


def _raw(value: object, document: Evaluator, name: str, dimension: tuple[int, int], plain_factor: float) -> float:
    """A raw value of `dimension` in internal units (see `raw_length` and `raw_angle`)."""
    if _is_number(value):
        return raw_number(value, name) * plain_factor
    if isinstance(value, Expr) and value._quantity is not None:
        number, symbol = value._quantity
        factors = LENGTH_FACTORS if dimension == LENGTH else ANGLE_FACTORS
        if symbol not in factors:
            unit_dimension = ANGLE if dimension == LENGTH else LENGTH
            raise ValueError(_dimension_message(name, dimension, value.source, unit_dimension))
        return float(number) * factors[symbol]
    if isinstance(value, (Expr, str)):
        source = value.source if isinstance(value, Expr) else value
        evaluated, evaluated_dimension = document._evaluate(source)
        if evaluated_dimension == dimension:
            return evaluated
        if evaluated_dimension == PLAIN:
            return evaluated * plain_factor
        raise ValueError(_dimension_message(name, dimension, source, evaluated_dimension))
    kind = "a length" if dimension == LENGTH else "an angle"
    raise TypeError(f"{name} must be {kind}: a number, an Expr or expression text, not {type(value).__name__}.")


def _dimension_message(name: str, expected: tuple[int, int], source: str, actual: tuple[int, int]) -> str:
    """The message for a raw value of the wrong dimension."""
    return f"{name} must be {describe_dimension(expected)}, but {source!r} is {describe_dimension(actual)}."

