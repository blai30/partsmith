"""STL, 3MF, OBJ and STEP export (python-api.md sections 2.9 and 3.7)."""

from __future__ import annotations

import os
from collections.abc import Sequence
from pathlib import Path

from ._files import write_atomic
from .document import Document, Part, warn
from .errors import PartsmithWarning
from .expressions import DEGREE, raw_number
from .selectors import Axis, rotation_rows

Rotations = Sequence[tuple[Axis, float]]


def export_stl(
    parts: Part | Sequence[Part],
    path: str | os.PathLike[str],
    *,
    ascii: bool = False,
    per_part: bool = False,
    chord: float | None = None,
    angle: float | None = None,
    rotate: Rotations = (),
) -> list[Path]:
    """Writes the parts as one STL file, or with `per_part` one file per part into the directory
    `path`; returns the written paths in part order."""
    stl = {"kind": "stl", "binary": not ascii, "per_body": per_part}
    return _export("export_stl", parts, path, stl, chord, angle, rotate, per_part)


def export_3mf(
    parts: Part | Sequence[Part],
    path: str | os.PathLike[str],
    *,
    chord: float | None = None,
    angle: float | None = None,
    rotate: Rotations = (),
) -> list[Path]:
    """Writes the parts as objects of one 3MF file."""
    return _export("export_3mf", parts, path, {"kind": "3mf"}, chord, angle, rotate, False)


def export_obj(
    parts: Part | Sequence[Part],
    path: str | os.PathLike[str],
    *,
    chord: float | None = None,
    angle: float | None = None,
    rotate: Rotations = (),
) -> list[Path]:
    """Writes the parts as objects of one OBJ file."""
    return _export("export_obj", parts, path, {"kind": "obj"}, chord, angle, rotate, False)


def export_step(parts: Part | Sequence[Part], path: str | os.PathLike[str], *, rotate: Rotations = ()) -> list[Path]:
    """Writes the exact B-rep of the parts as one STEP file."""
    return _export("export_step", parts, path, {"kind": "step"}, None, None, rotate, False)


def _export(
    function: str,
    parts: Part | Sequence[Part],
    path: str | os.PathLike[str],
    file_format: dict,
    chord: float | None,
    angle: float | None,
    rotate: Rotations,
    into_directory: bool,
) -> list[Path]:
    """Sends `export` and writes each file it returns atomically."""
    selected = [parts] if isinstance(parts, Part) else list(parts)
    if not selected:
        raise ValueError(f"{function}() needs at least one part.")
    if not isinstance(selected[0], Part):
        raise TypeError(f"{function}() takes parts, not {type(selected[0]).__name__}.")
    document = selected[0].document
    bodies = [document._own_part(part, f"{function}()").id for part in selected]
    tolerance = None if chord is None and angle is None else _tolerance(document, function, chord, angle)
    orientation = (
        {"kind": "matrix", "rows": rotation_rows(rotate, document, f"{function}() rotate")} if rotate else None
    )
    reply = document._client.command(
        "export", format=file_format, bodies=bodies, tolerance=tolerance, orientation=orientation
    )
    for warning in reply.result["warnings"]:
        warn(PartsmithWarning(warning["message"], code=warning["code"]))
    files = reply.result["files"]
    target = Path(path)
    if into_directory:
        target.mkdir(parents=True, exist_ok=True)
        written = [target / file["name"] for file in files]
    else:
        written = [target]
    for file_path, file in zip(written, files, strict=True):
        write_atomic(file_path, reply.buffers[file["data"]])
    return written


def _tolerance(document: Document, function: str, chord: float | None, angle: float | None) -> dict:
    """The explicit tolerance: `chord` in mm and `angle` in degrees, the missing one from the
    document's standard tolerance."""
    tolerance = {}
    if chord is not None:
        tolerance["chord_deviation"] = raw_number(chord, f"{function}() chord")
    if angle is not None:
        tolerance["angular_deviation"] = raw_number(angle, f"{function}() angle") * DEGREE
    if len(tolerance) < 2:
        # Python documents always display at standard quality, so this reads the resolved
        # standard tolerance without changing anything.
        standard = document._client.command("set_tessellation", quality="standard").result["tolerance"]
        tolerance = {**standard, **tolerance}
    return tolerance
