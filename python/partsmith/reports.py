"""Measurement and print check results (python-api.md section 2.9)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from .expressions import triple

if TYPE_CHECKING:
    from collections.abc import Callable

    from .document import Part

Point3 = tuple[float, float, float]

AXIS_TOLERANCE = 1e-9
"""A split plane whose normal is within this of a model axis reads as that axis (print.md section 3.12)."""

AXIS_NAMES = ("x", "y", "z")


@dataclass(frozen=True)
class BoundingBox:
    """An axis-aligned box in millimeters."""

    min: Point3
    max: Point3


@dataclass(frozen=True)
class SplitPlane:
    """A plane a split suggestion cuts along, in model coordinates (millimeters)."""

    origin: Point3
    """A point on the plane."""
    normal: Point3
    """The plane's unit normal."""


@dataclass(frozen=True)
class Suggestion:
    """A print.md suggested fix: split the part along planes, or add a drain hole near a point."""

    kind: str
    """"split" or "drain_hole"."""
    text: str
    """The suggestion in words, in millimeters (print.md section 3.12), for example "Split at x = 150 mm"."""
    planes: tuple[SplitPlane, ...]
    """The split planes in order; () for a drain hole."""
    at: Point3 | None
    """The drain hole's point; None for a split."""


@dataclass(frozen=True)
class Finding:
    """One print.md finding, with its part."""

    check: str
    """The check that produced it (a CheckKind value, for example "overhangs")."""
    code: str
    """What kind of problem it is (a FindingCode value, for example "overhang")."""
    severity: str
    """"info", "warning" or "error"."""
    part: Part
    """The part it is about."""
    message: str
    """A sentence naming the problem and the fix."""
    location: Point3
    """The center of the finding's bounds, or its point, in model coordinates."""
    value: float | None
    """The measured value, when there is one."""
    limit: float | None
    """The limit the value was compared with, when there is one."""
    suggestion: Suggestion | None
    """The finding's suggested fix, when it has one."""


@dataclass(frozen=True)
class PrintReport:
    """The result of `Document.check`."""

    profile: str
    """The printer profile the checks used."""
    health: str
    """"ok", "warning" or "error"."""
    label: str
    """The badge text, for example "Ready to print"."""
    findings: list[Finding]
    """Every finding in body order, then check order, then each check's own order."""


def bounding_box(data: dict) -> BoundingBox:
    """The bounding box of a protocol `{min, max}` object."""
    return BoundingBox(min=point(data["min"]), max=point(data["max"]))


def point(values: list[float]) -> Point3:
    """A protocol `Vec3` as a tuple."""
    return triple(values)


def print_report(report: dict, part_of: Callable[[str], Part]) -> PrintReport:
    """The PrintReport of a protocol `PrintHealthReport`; `part_of` gives the part of a body id."""
    findings = [
        Finding(
            check=finding["check"],
            code=finding["code"],
            severity=finding["severity"],
            part=part_of(body["body"]),
            message=finding["message"],
            location=location_center(finding["location"]),
            value=finding.get("value"),
            limit=finding.get("limit"),
            suggestion=suggestion(finding.get("suggestion")),
        )
        for body in report["bodies"]
        for check in body["report"]["checks"]
        for finding in check["findings"]
    ]
    summary = report["summary"]
    return PrintReport(profile=report["profile"], health=summary["health"], label=summary["label"], findings=findings)


def location_center(location: dict) -> Point3:
    """The center of a finding's location: the middle of its bounds, or its point."""
    if location["kind"] == "point":
        return point(location["at"])
    low, high = location["min"], location["max"]
    return point([(low[i] + high[i]) / 2 for i in range(3)])


def suggestion(data: dict | None) -> Suggestion | None:
    """The Suggestion of a protocol suggestion; None without one, or for a split without planes, which has no words."""
    if data is None:
        return None
    if data["kind"] == "drain_hole":
        at = point(data["at"])
        return Suggestion(kind="drain_hole", text=f"Add a drain hole near ({coordinates(at)} mm)", planes=(), at=at)
    planes = tuple(SplitPlane(origin=point(plane["origin"]), normal=point(plane["normal"])) for plane in data["planes"])
    if not planes:
        return None
    *others, last = [plane_words(plane) for plane in planes]
    text = f"Split at {', '.join(others)} and {last}" if others else f"Split at {last}"
    return Suggestion(kind="split", text=text, planes=planes, at=None)


def plane_words(plane: SplitPlane) -> str:
    """The words of one plane: "x = 150 mm" along a model axis, else its origin and normal (print.md section 3.12)."""
    for axis, name in enumerate(AXIS_NAMES):
        if abs(plane.normal[axis]) >= 1 - AXIS_TOLERANCE:
            return f"{name} = {length(plane.origin[axis])} mm"
    return f"the plane through ({coordinates(plane.origin)} mm) with normal ({coordinates(plane.normal)})"


def coordinates(values: Point3) -> str:
    """The three values written with `length` and joined by ", "."""
    return ", ".join(length(value) for value in values)


def length(value: float) -> str:
    """`value` with at most three decimals, trailing zeros and a trailing "." removed, and -0 written 0 (print.md's
    `len`, section 3.9.0)."""
    text = f"{value:.3f}".rstrip("0").rstrip(".")
    return "0" if text == "-0" else text
