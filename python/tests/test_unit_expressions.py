"""Unit tests of expression text, units, the printer namespace and raw value conversion in
partsmith.expressions."""

import math

import pytest

from partsmith import cm, deg, expr, inch, meter, mm, printer, rad
from partsmith.expressions import (
    ATOM,
    POWER,
    PRODUCT,
    SUM,
    UNARY,
    is_quantity,
    raw_angle,
    raw_length,
    raw_number,
    render_value,
    source_precedence,
)


class FakeDocument:
    """A document stand-in whose `_evaluate` returns a fixed result."""

    def __init__(self, units: str = "mm", result: tuple[float, tuple[int, int]] = (0.0, (0, 0))) -> None:
        self.units = units
        self.result = result
        self.evaluated: list[str] = []

    def _evaluate(self, source: str) -> tuple[float, tuple[int, int]]:
        self.evaluated.append(source)
        return self.result


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        (40, "40"),
        (-5, "-5"),
        (40.0, "40"),
        (-0.0, "0"),
        (0.1, "0.1"),
        (1e-05, "1e-05"),
        (2.0**53, "9007199254740992.0"),
        ("2*height + 1", "2*height + 1"),
        (expr("wall"), "wall"),
    ],
)
def test_values_render_as_expression_text(value, expected):
    assert render_value(value, "x") == expected


@pytest.mark.parametrize("value", [float("nan"), float("inf"), -float("inf")])
def test_non_finite_numbers_are_refused(value):
    with pytest.raises(ValueError, match="x must be a finite number"):
        render_value(value, "x")


@pytest.mark.parametrize("value", [True, None, [1], printer])
def test_other_types_are_refused(value):
    with pytest.raises(TypeError, match="x must be a number, an Expr or expression text"):
        render_value(value, "x")


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda: 2 * mm, "2 mm"),
        (lambda: mm * 2, "2 mm"),
        (lambda: 1.5 * inch, "1.5 in"),
        (lambda: 10 * meter, "10 m"),
        (lambda: 2.0 * cm, "2 cm"),
        (lambda: -2 * mm, "-2 mm"),
        (lambda: 30 * deg, "30 deg"),
        (lambda: 1.5 * rad, "1.5 rad"),
        (lambda: mm, "1 mm"),
    ],
)
def test_a_number_times_a_unit_is_a_quantity_literal(build, expected):
    quantity = build()

    assert str(quantity) == expected
    assert is_quantity(quantity)


@pytest.mark.parametrize("value", [expr("2 mm"), 2 * mm * 3, -(2 * mm), mm * expr("wall"), "2 mm", 2], ids=str)
def test_other_values_are_not_quantity_literals(value):
    assert not is_quantity(value)


@pytest.mark.parametrize(
    ("build", "expected"),
    [
        (lambda a, b: a + b * 2, "a + b * 2"),
        (lambda a, b: (a + 1) * 2, "(a + 1) * 2"),
        (lambda a, b: a - (b - 1), "a - (b - 1)"),
        (lambda a, b: a - (b + 1), "a - (b + 1)"),
        (lambda a, b: (a - b) - 1, "a - b - 1"),
        (lambda a, b: a / (b * 2), "a / (b * 2)"),
        (lambda a, b: a * (b / 2), "a * b / 2"),
        (lambda a, b: a**b**2, "a ^ b ^ 2"),
        (lambda a, b: (a**b) ** 2, "(a ^ b) ^ 2"),
        (lambda a, b: -(a**2), "-a ^ 2"),
        (lambda a, b: -(a + b), "-(a + b)"),
        (lambda a, b: -(a * b), "-(a * b)"),
        (lambda a, b: a**-2, "a ^ (-2)"),
        (lambda a, b: (-2) ** a, "(-2) ^ a"),
        (lambda a, b: 2 - a, "2 - a"),
        (lambda a, b: 1 / a, "1 / a"),
        (lambda a, b: +a, "a"),
        (lambda a, b: a * "b + 1", "a * (b + 1)"),
        (lambda a, b: "b + 1" - a, "b + 1 - a"),
        (lambda a, b: 1 - expr("-x"), "1 - -x"),
        (lambda a, b: 2 * mm * a, "2 mm * a"),
        (lambda a, b: a / (2 * mm), "a / 2 mm"),
        (lambda a, b: a * 0.5, "a * 0.5"),
    ],
)
def test_operators_add_only_the_parentheses_that_keep_the_meaning(build, expected):
    assert str(build(expr("a"), expr("b"))) == expected


def test_an_unsupported_operand_raises_type_error():
    with pytest.raises(TypeError):
        expr("a") + {1}


def test_a_bool_operand_raises_type_error():
    with pytest.raises(TypeError):
        expr("a") * True


@pytest.mark.parametrize(
    ("source", "expected"),
    [
        ("2 mm", ATOM),
        ("printer.fit.slip", ATOM),
        ("sqrt(a + b)", ATOM),
        ("(a + b)", ATOM),
        ("1e-5", ATOM),
        ("30°", ATOM),
        ("a ^ 2", POWER),
        ("a ^ -2", POWER),
        ("-a", UNARY),
        ("-a ^ 2", UNARY),
        ("a * b", PRODUCT),
        ("-a * b", PRODUCT),
        ("a / 2 mm", PRODUCT),
        ("a + b", SUM),
        ("a - -b", SUM),
        ("2 $", SUM),
        ("(a", SUM),
        ("   ", SUM),
    ],
)
def test_the_precedence_of_text_is_its_top_level_operation(source, expected):
    assert source_precedence(source) == expected


def test_expr_keeps_its_text_verbatim():
    assert str(expr(" 2*wall ")) == " 2*wall "


def test_expr_repr_shows_the_call():
    assert repr(expr("2*wall")) == "expr('2*wall')"


def test_expr_needs_text():
    with pytest.raises(TypeError):
        expr(5)


def test_printer_attributes_build_dotted_names():
    assert str(printer.fit.slip) == "printer.fit.slip"
    assert str(printer.nozzle) == "printer.nozzle"


def test_printer_names_starting_with_underscore_do_not_exist():
    with pytest.raises(AttributeError):
        _ = printer._hidden
    with pytest.raises(AttributeError):
        _ = printer.fit._hidden


def test_printer_alone_is_not_a_value():
    with pytest.raises(TypeError):
        printer + 1


@pytest.mark.parametrize(("units", "expected"), [("mm", 2.0), ("cm", 20.0), ("m", 2000.0), ("in", 50.8)])
def test_a_plain_length_is_in_document_units(units, expected):
    assert raw_length(2, FakeDocument(units), "x") == pytest.approx(expected, rel=1e-15)


def test_a_length_quantity_converts_by_its_unit():
    document = FakeDocument("in")

    assert raw_length(5 * mm, document, "x") == 5.0
    assert raw_length(1.5 * inch, document, "x") == 1.5 * 25.4
    assert document.evaluated == []


def test_an_angle_quantity_is_not_a_length():
    with pytest.raises(ValueError, match=r"^x must be a length, but '30 deg' is an angle\.$"):
        raw_length(30 * deg, FakeDocument(), "x")


def test_other_expressions_are_evaluated_by_the_engine():
    document = FakeDocument(result=(12.5, (1, 0)))

    assert raw_length(expr("wall * 2"), document, "x") == 12.5
    assert document.evaluated == ["wall * 2"]


def test_a_plain_evaluated_length_is_in_document_units():
    assert raw_length("2 + 3", FakeDocument("cm", result=(5.0, (0, 0))), "x") == 50.0


def test_an_evaluated_angle_is_not_a_length():
    with pytest.raises(ValueError, match="is an angle"):
        raw_length("a", FakeDocument(result=(1.0, (0, 1))), "x")


def test_a_bool_is_not_a_length():
    with pytest.raises(TypeError, match="x must be a length"):
        raw_length(True, FakeDocument(), "x")


def test_a_plain_angle_is_in_degrees():
    assert raw_angle(90, FakeDocument(), "x") == math.pi / 2


def test_an_angle_quantity_converts_by_its_unit():
    assert raw_angle(1.5 * rad, FakeDocument(), "x") == 1.5
    assert raw_angle(30 * deg, FakeDocument(), "x") == 30 * math.pi / 180


def test_a_plain_evaluated_angle_is_in_degrees():
    assert raw_angle("45", FakeDocument(result=(45.0, (0, 0))), "x") == 45 * math.pi / 180


def test_raw_numbers_are_finite_floats():
    assert raw_number(3, "x") == 3.0
    with pytest.raises(TypeError):
        raw_number(False, "x")
    with pytest.raises(ValueError):
        raw_number(float("inf"), "x")
