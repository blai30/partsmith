"""Measurement and print check results (python-api.md section 2.9)."""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING

from .expressions import triple

if TYPE_CHECKING:
    from collections.abc import Callable

    from .document import Part

Point3 = tuple[float, float, float]


@dataclass(frozen=True)
class BoundingBox:
    """An axis-aligned box in millimeters."""

    min: Point3
    max: Point3


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
